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
    /// Endpoint domains that were verified against the certificate.
    pub validated_endpoint_domains: Vec<String>,
}

/// Uploads (or replaces) an SSL certificate in the running cache by id.
///
/// Safety checks performed before anything is stored:
/// 1. The PEM material must parse and the private key must be of a supported type.
/// 2. At least one https/mcp endpoint must reference `cert_id` — we refuse to store a
///    certificate that protects no endpoint (guards against typos / orphan uploads).
/// 3. The certificate must cover the domain (SNI server name) of every endpoint that
///    references `cert_id` — we refuse to serve a certificate issued for a different domain.
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

    // (3) The certificate must cover every endpoint domain it is being installed for.
    let cert_domains = ssl_cert.get_domains();

    let mut validated_endpoint_domains: Vec<String> = Vec::new();
    let mut uncovered: Vec<String> = Vec::new();

    for endpoint in &referencing {
        let Some(server_name) = endpoint.server_name.as_deref() else {
            // Default endpoint without SNI — no domain to validate against.
            continue;
        };

        if cert_covers_domain(&cert_domains, server_name) {
            if !validated_endpoint_domains
                .iter()
                .any(|d| d.eq_ignore_ascii_case(server_name))
            {
                validated_endpoint_domains.push(server_name.to_string());
            }
        } else if !uncovered.iter().any(|d| d.eq_ignore_ascii_case(server_name)) {
            uncovered.push(server_name.to_string());
        }
    }

    if !uncovered.is_empty() {
        return Err(format!(
            "Uploaded certificate does not cover endpoint domain(s): [{}]. The certificate is valid for: [{}]. Refusing upload — this looks like a certificate for a different domain.",
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
        validated_endpoint_domains,
    })
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
        return match domain.split_once('.') {
            Some((_, rest)) => rest.eq_ignore_ascii_case(suffix),
            None => false,
        };
    }

    pattern.eq_ignore_ascii_case(domain)
}
