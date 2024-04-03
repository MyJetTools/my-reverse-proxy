use crate::{
    app::AppContext,
    app_configuration::{AppConfiguration, SELF_SIGNED_CERT_NAME},
    ssl::*,
};

pub async fn get_and_check_app_config(app: &AppContext) -> Result<AppConfiguration, String> {
    let settings_reader = crate::settings::SettingsReader::new(".my-reverse-proxy").await;
    let listen_ports = settings_reader.get_listen_ports(app).await?;

    let mut ssl_certificates_cache = SslCertificatesCache::new();

    let mut client_certificates_cache = ClientCertificatesCache::new();

    for (listen_port, port_config) in &listen_ports {
        if let Some(ssl_cert_id) = port_config.get_ssl_certificate() {
            if ssl_cert_id.as_str() == SELF_SIGNED_CERT_NAME {
                continue;
            }
            if !ssl_certificates_cache.has_certificate(ssl_cert_id) {
                let ssl_certificate =
                    crate::flows::load_ssl_certificate(&settings_reader, ssl_cert_id, *listen_port)
                        .await?;
                ssl_certificates_cache.add(ssl_cert_id, ssl_certificate);
            }
        }

        for client_cert_id in port_config.get_client_certificates() {
            println!(
                "Port: {} has client certificate {}",
                *listen_port,
                client_cert_id.as_str()
            );
            if !client_certificates_cache.has_certificate(client_cert_id) {
                let client_certificate = crate::flows::load_client_certificate(
                    &settings_reader,
                    client_cert_id,
                    *listen_port,
                )
                .await?;

                client_certificates_cache.insert(client_cert_id, client_certificate);
            }
        }
    }

    Ok(AppConfiguration {
        listen_ports,
        ssl_certificates_cache,
        client_certificates_cache,
    })
}
