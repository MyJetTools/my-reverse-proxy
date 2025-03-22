use crate::{configurations::SslCertificateIdRef, settings_compiled::SettingsCompiled};

pub async fn refresh_ca_from_settings(cert_id: &str) -> Result<(), String> {
    let settings_model = SettingsCompiled::load_settings().await?;

    let client_ca_id = SslCertificateIdRef::new(cert_id);
    crate::scripts::refresh_ca_from_sources(&settings_model, client_ca_id).await?;

    Ok(())
}
