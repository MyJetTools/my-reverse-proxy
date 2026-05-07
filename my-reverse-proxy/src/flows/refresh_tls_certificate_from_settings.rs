use crate::{configurations::SslCertificateIdRef, settings_compiled::SettingsCompiled};

pub async fn refresh_tls_certificate_from_settings(cert_id: &str) -> Result<(), String> {
    let settings_model = SettingsCompiled::load_settings().await?;

    let ssl_cert_id = SslCertificateIdRef::new(cert_id);
    crate::scripts::refresh_ssl_certs_from_sources(&settings_model, ssl_cert_id).await?;

    Ok(())
}
