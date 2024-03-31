use std::sync::Arc;

use crate::{app::AppContext, http_server::ClientCertificateCa, settings::SslCertificateId};

pub async fn get_client_certificate(
    app: &AppContext,
    cert_id: &SslCertificateId,
    listen_port: u16,
) -> Result<Arc<ClientCertificateCa>, String> {
    if let Some(result) = app.client_certificates.get(cert_id.as_str()).await {
        return Ok(result);
    }

    if let Some(client_cert) = app
        .settings_reader
        .get_client_certificate_ca(cert_id.as_str())
        .await
        .unwrap()
    {
        let client_ca = crate::flows::get_file(&client_cert).await;
        let client_ca: Arc<ClientCertificateCa> = Arc::new(client_ca.into());

        app.client_certificates
            .insert_if_not_exists(cert_id.as_str(), client_ca.clone())
            .await;

        return Ok(client_ca);
    }

    return Err(format!(
        "Client certificate ca not found: {} for endpoint: {}",
        cert_id.as_str(),
        listen_port
    ));
}
