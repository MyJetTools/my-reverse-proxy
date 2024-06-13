use std::sync::Arc;

use crate::{
    http_server::ClientCertificateCa,
    settings::{SettingsModel, SslCertificateId},
};

pub async fn load_client_certificate(
    settings_model: &SettingsModel,
    cert_id: &SslCertificateId,
    listen_port: u16,
) -> Result<Arc<ClientCertificateCa>, String> {
    let cert_result = settings_model.get_client_certificate_ca(cert_id.as_str())?;

    if cert_result.is_none() {
        return Err(format!(
            "Client certificate ca  {} not found for endpoint: {}",
            cert_id.as_str(),
            listen_port
        ));
    }

    let cert_result = cert_result.unwrap();

    let client_ca = cert_result.load_file_content().await;

    let client_ca: Arc<ClientCertificateCa> = Arc::new(client_ca.into());
    return Ok(client_ca);
}
