use std::collections::BTreeMap;

use crate::{
    app::AppContext,
    app_configuration::{AppConfiguration, SELF_SIGNED_CERT_NAME},
    files_cache::FilesCache,
    ssl::*,
};

pub async fn get_and_check_app_config(app: &AppContext) -> Result<AppConfiguration, String> {
    let settings_model = crate::settings::SettingsModel::load(".my-reverse-proxy").await?;
    let listen_ports = settings_model.get_listen_ports(app).await?;

    let mut ssl_certificates_cache = SslCertificatesCache::new();

    let mut client_certificates_cache = ClientCertificatesCache::new();

    let mut http_endpoints = BTreeMap::new();

    let mut tcp_endpoints = BTreeMap::new();

    let mut tcp_over_ssh_endpoints = BTreeMap::new();

    let files_cache = FilesCache::new();

    for (listen_port, port_config) in listen_ports {
        match port_config {
            crate::app_configuration::ListenPortConfiguration::Http(port_config) => {
                if let Some(ssl_certs) = port_config.get_ssl_certificates() {
                    for ssl_cert_id in ssl_certs {
                        if ssl_cert_id.as_str() != SELF_SIGNED_CERT_NAME {
                            if !ssl_certificates_cache.has_certificate(ssl_cert_id) {
                                let ssl_certificate = crate::flows::load_ssl_certificate(
                                    &settings_model,
                                    ssl_cert_id,
                                    listen_port,
                                    &files_cache,
                                )
                                .await?;
                                ssl_certificates_cache.add(ssl_cert_id, ssl_certificate);
                            }
                        }
                    }
                }

                for endpoint_info in &port_config.endpoint_info {
                    if let Some(client_cert_id) = endpoint_info.client_certificate_id.as_ref() {
                        if !client_certificates_cache.has_certificate(client_cert_id) {
                            let client_certificate = crate::flows::load_client_certificate(
                                &settings_model,
                                client_cert_id,
                                listen_port,
                                &files_cache,
                            )
                            .await?;

                            client_certificates_cache.insert(client_cert_id, client_certificate);
                        }
                    }
                }

                http_endpoints.insert(listen_port, port_config);
            }
            crate::app_configuration::ListenPortConfiguration::Tcp(port_config) => {
                tcp_endpoints.insert(listen_port, port_config);
            }
            crate::app_configuration::ListenPortConfiguration::TcpOverSsh(port_config) => {
                tcp_over_ssh_endpoints.insert(listen_port, port_config);
            }
        }
    }

    Ok(AppConfiguration {
        http_endpoints,
        tcp_endpoints,
        tcp_over_ssh_endpoints,
        ssl_certificates_cache,
        client_certificates_cache,
    })
}
