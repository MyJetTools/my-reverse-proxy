use crate::{
    settings::{SettingsModel, SslCertificateId},
    ssl::SslCertificate,
};

pub async fn load_ssl_certificate(
    settings_model: &SettingsModel,
    ssl_id: &SslCertificateId,
    listen_port: u16,
) -> Result<SslCertificate, String> {
    let cert_result = settings_model.get_ssl_certificate(ssl_id)?;

    if cert_result.is_none() {
        return Err(format!(
            "SSL certificate {} not found for https port {}",
            ssl_id.as_str(),
            listen_port
        ));
    }

    let (cert, key) = cert_result.unwrap();

    let certificates = crate::flows::get_file(&cert).await;
    let private_key = crate::flows::get_file(&key).await;

    let ssl_certificate = SslCertificate::new(certificates, private_key, key.as_str().as_str());

    Ok(ssl_certificate)
}
