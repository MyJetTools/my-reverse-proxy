use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    configurations::SslCertificateIdRef,
    ssl::{SslCertificate, SslCertificateOrigin},
};

pub struct InitSslCertResult {
    pub cert_id: String,
    pub cn: String,
    pub expires: DateTimeAsMicroseconds,
    /// Domains (SAN + CN) the uploaded certificate is valid for.
    pub covered_domains: Vec<String>,
    /// The SNI(s) that were required and verified to be covered by the uploaded certificate.
    pub validated_sni: Vec<String>,
}

struct RequiredSni {
    domain: String,
    /// Where this SNI requirement came from — for a clear rejection message.
    source: &'static str,
}

/// Uploads (or replaces) an SSL certificate in the running cache by id.
///
/// Safety checks performed before anything is stored:
/// 1. The PEM material must parse and the private key must match the certificate (see
///    [`SslCertificate::new`]).
/// 2. At least one https/mcp endpoint must reference `cert_id` — we refuse to store a
///    certificate that protects no endpoint (guards against typos / orphan uploads).
/// 3. The uploaded certificate must cover every required SNI — i.e. it must serve the same
///    domain(s) as (a) each referencing endpoint's configured server name AND (b) the
///    certificate currently loaded under `cert_id` (if any). This is the core guard against
///    accidentally installing a certificate for an unrelated domain: a replacement must match
///    the SNI of what it replaces, and this holds even for endpoints with no configured
///    server name.
///
/// The stored certificate is marked [`SslCertificateOrigin::ManuallyProvided`], so the
/// renewal timer leaves it untouched. It is served on the next TLS handshake — no reload
/// required. To hand management back to a configured source, use RefreshSslCertificate.
pub async fn init_ssl_cert_manually(
    cert_id: &str,
    cert_pem: Vec<u8>,
    private_key_pem: Vec<u8>,
) -> Result<InitSslCertResult, String> {
    let cert_id = cert_id.trim();

    if cert_id.is_empty() {
        return Err("cert_id must not be empty".to_string());
    }

    let ssl_cert_id = SslCertificateIdRef::new(cert_id);

    if ssl_cert_id.is_self_signed() {
        return Err(format!(
            "'{}' is the reserved self-signed certificate id and cannot be set manually",
            cert_id
        ));
    }

    // (1) Parse + validate the PEM material. Rejects malformed certificates and
    // unsupported/broken private keys instead of panicking on bad input.
    let ssl_cert = SslCertificate::new(private_key_pem.clone(), cert_pem.clone())?;

    // (2) The certificate must actually protect an endpoint.
    let referencing: Vec<_> = crate::scripts::get_ssl_endpoints_status()
        .await
        .into_iter()
        .filter(|endpoint| endpoint.cert_id == cert_id)
        .collect();

    if referencing.is_empty() {
        return Err(format!(
            "No https endpoint references ssl certificate id '{}'. Refusing to upload a certificate that protects no endpoint.",
            cert_id
        ));
    }

    // (3) The certificate must serve the same SNI(s) as what it protects / replaces.
    let cert_domains = ssl_cert.get_domains();

    // A certificate with no identifiable domain can never be validated against an SNI.
    if cert_domains.is_empty() {
        return Err(
            "The uploaded certificate has no SAN DNS name or Common Name — refusing a certificate with no identifiable domain."
                .to_string(),
        );
    }

    // The domains the certificate currently loaded under this id is valid for. A replacement
    // must keep covering these — this is what makes the new and existing SNIs coincide, and it
    // catches an unrelated certificate even for an endpoint that has no configured server name.
    let existing_domains = crate::app::APP_CTX
        .ssl_certificates_cache
        .read(|c| c.ssl_certs.get(SslCertificateIdRef::new(cert_id)))
        .await
        .map(|holder| holder.ssl_cert.get_domains())
        .unwrap_or_default();

    let mut required: Vec<RequiredSni> = Vec::new();
    for endpoint in &referencing {
        if let Some(server_name) = endpoint.server_name.as_deref() {
            push_required(&mut required, server_name, "endpoint server name");
        }
    }
    for domain in &existing_domains {
        push_required(&mut required, domain, "existing certificate");
    }

    // Fail closed: with no SNI to validate against (every referencing endpoint is a catch-all
    // with no server name AND nothing is loaded under this id yet) we cannot establish that the
    // certificate belongs to the endpoint, so we refuse it rather than accept an arbitrary cert.
    if required.is_empty() {
        return Err(format!(
            "Cannot verify the domain of certificate '{}': its referencing endpoint(s) have no configured server name and no certificate is currently loaded under this id to compare against. Configure a server name on the endpoint (or establish the certificate from a configured source first) before uploading.",
            cert_id
        ));
    }

    let mut validated_sni: Vec<String> = Vec::new();
    let mut uncovered: Vec<String> = Vec::new();
    for req in &required {
        if cert_covers_domain(&cert_domains, &req.domain) {
            validated_sni.push(req.domain.clone());
        } else {
            uncovered.push(format!("{} (required by {})", req.domain, req.source));
        }
    }

    if !uncovered.is_empty() {
        return Err(format!(
            "Uploaded certificate does not cover the required SNI(s): [{}]. The certificate is valid for: [{}]. Refusing upload — the new certificate must serve the same domain(s) as the endpoint and the certificate it replaces.",
            uncovered.join(", "),
            if cert_domains.is_empty() {
                "<none>".to_string()
            } else {
                cert_domains.join(", ")
            }
        ));
    }

    let cert_info = ssl_cert.get_cert_info();

    crate::app::APP_CTX
        .ssl_certificates_cache
        .write(|config| {
            config.ssl_certs.add_or_update(
                ssl_cert_id,
                ssl_cert,
                SslCertificateOrigin::ManuallyProvided,
                cert_pem,
                private_key_pem,
            );
        })
        .await;

    println!(
        "SSL certificate '{}' has been set manually (cn: {}, expires: {}).",
        cert_id,
        cert_info.cn,
        cert_info.expires.to_rfc3339()
    );

    Ok(InitSslCertResult {
        cert_id: cert_id.to_string(),
        cn: cert_info.cn,
        expires: cert_info.expires,
        covered_domains: cert_domains,
        validated_sni,
    })
}

/// Adds `domain` to the required-SNI list unless an equal (case-insensitive) entry is already
/// present. The first source that introduced the domain is kept for the rejection message.
fn push_required(required: &mut Vec<RequiredSni>, domain: &str, source: &'static str) {
    if required
        .iter()
        .any(|req| req.domain.eq_ignore_ascii_case(domain))
    {
        return;
    }

    required.push(RequiredSni {
        domain: domain.to_string(),
        source,
    });
}

/// Whether any of the certificate's domains matches `domain`, honouring single-label
/// wildcards (`*.example.com` matches `api.example.com`, but not `example.com` or
/// `a.b.example.com`).
fn cert_covers_domain(cert_domains: &[String], domain: &str) -> bool {
    cert_domains
        .iter()
        .any(|pattern| wildcard_match(pattern, domain))
}

fn wildcard_match(pattern: &str, domain: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("*.") {
        // Reject a wildcard that spans a whole TLD, e.g. `*.com`: the base must have at least
        // two labels (contain a dot), otherwise an over-broad cert would match any two-label host.
        if !suffix.contains('.') {
            return false;
        }

        return match domain.split_once('.') {
            Some((_, rest)) => rest.eq_ignore_ascii_case(suffix),
            None => false,
        };
    }

    pattern.eq_ignore_ascii_case(domain)
}

#[cfg(test)]
mod tests {
    use super::{cert_covers_domain, wildcard_match};

    #[test]
    fn exact_match_is_case_insensitive() {
        assert!(wildcard_match("example.com", "example.com"));
        assert!(wildcard_match("Example.COM", "example.com"));
        assert!(!wildcard_match("example.com", "other.com"));
    }

    #[test]
    fn wildcard_matches_exactly_one_label() {
        assert!(wildcard_match("*.example.com", "api.example.com"));
        assert!(wildcard_match("*.example.com", "API.example.com"));
        // wildcard does not match the bare base...
        assert!(!wildcard_match("*.example.com", "example.com"));
        // ...nor more than one label deep.
        assert!(!wildcard_match("*.example.com", "a.b.example.com"));
    }

    #[test]
    fn tld_wide_wildcard_is_rejected() {
        assert!(!wildcard_match("*.com", "myapp.com"));
        assert!(!wildcard_match("*.io", "app.io"));
    }

    #[test]
    fn bare_star_does_not_over_match() {
        assert!(!wildcard_match("*", "example.com"));
    }

    #[test]
    fn cert_covers_domain_uses_any_pattern() {
        let domains = vec!["*.example.com".to_string(), "example.com".to_string()];
        assert!(cert_covers_domain(&domains, "api.example.com"));
        assert!(cert_covers_domain(&domains, "example.com"));
        assert!(!cert_covers_domain(&domains, "example.org"));
        assert!(!cert_covers_domain(&[], "example.com"));
    }
}
