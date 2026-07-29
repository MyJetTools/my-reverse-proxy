use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::configurations::SslCertificateIdRef;

pub struct SslCertificateDetails {
    pub id: String,
    pub cn: String,
    pub expires: DateTimeAsMicroseconds,
    /// Where the certificate came from: `local_source`, `gateway_pushed` or `manually_provided`.
    pub origin: String,
    /// Domains (SAN DNS names + CN) the certificate is valid for.
    pub domains: Vec<String>,
}

/// Returns metadata about the currently loaded certificate for the given id, or `None`
/// when nothing is loaded under that id. Neither the certificate PEM material nor the
/// private key is ever returned — only metadata.
pub async fn get_ssl_certificate_details(cert_id: &str) -> Option<SslCertificateDetails> {
    let cert_id = cert_id.trim();

    let holder = crate::app::APP_CTX
        .ssl_certificates_cache
        .read(|c| c.ssl_certs.get(SslCertificateIdRef::new(cert_id)))
        .await?;

    let info = holder.ssl_cert.get_cert_info();
    let domains = holder.ssl_cert.get_domains();

    Some(SslCertificateDetails {
        id: cert_id.to_string(),
        cn: info.cn,
        expires: info.expires,
        origin: holder.origin.as_str().to_string(),
        domains,
    })
}
