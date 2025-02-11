use crate::configurations::SslCertificateIdRef;

pub async fn refresh_ca_from_settings(cert_id: &str) -> Result<(), String> {
    let settings_model = crate::scripts::load_settings().await?;

    let client_ca_id = SslCertificateIdRef::new(cert_id);
    crate::scripts::refresh_ca_from_sources(&settings_model, client_ca_id).await?;

    Ok(())
}
