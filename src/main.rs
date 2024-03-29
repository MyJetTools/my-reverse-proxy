use std::sync::Arc;

use app::{AppContext, SslCertificate};

mod app;
mod flows;
//mod http2_executor;
mod google_auth;
mod http_client;
mod http_content_source;
mod http_proxy_pass;
mod http_server;
mod populate_variable;
mod settings;
mod tcp_port_forward;
mod types;

#[tokio::main]
async fn main() {
    let settings_reader = settings::SettingsReader::new(".my-reverse-proxy").await;

    let listen_ports = settings_reader.get_listen_ports().await.unwrap();

    let app = AppContext::new(settings_reader).await;

    let app = Arc::new(app);

    let mut ssl_server_id = 0;

    for (listen_port, endpoint_type) in listen_ports {
        let listen_end_point = std::net::SocketAddr::from(([0, 0, 0, 0], listen_port));

        match endpoint_type {
            settings::EndpointType::Http1(endpoint_info) => {
                crate::http_server::start_http_server(listen_end_point, app.clone(), endpoint_info);
            }
            settings::EndpointType::Https {
                ssl_id,
                client_ca_id,
                endpoint_info,
            } => {
                if let Some((cert, private_key)) = app
                    .settings_reader
                    .get_ssl_certificate(&ssl_id)
                    .await
                    .unwrap()
                {
                    ssl_server_id += 1;

                    let ssl_certificate = SslCertificate::new(
                        crate::flows::get_file(&cert).await,
                        crate::flows::get_file(&private_key).await,
                        private_key.as_str().as_str(),
                    );

                    if let Some(client_ca_id) = client_ca_id {
                        if let Some(client_cert) = app
                            .settings_reader
                            .get_client_certificate_ca(client_ca_id.as_str())
                            .await
                            .unwrap()
                        {
                            let client_ca = crate::flows::get_file(&client_cert).await;
                            crate::http_server::start_https_server(
                                listen_end_point,
                                app.clone(),
                                ssl_certificate,
                                Some(client_ca.into()),
                                ssl_server_id,
                                endpoint_info,
                            );
                        } else {
                            panic!(
                                "Client certificate ca not found: {} for endpoint: {}",
                                client_ca_id.as_str(),
                                listen_port
                            );
                        }
                    } else {
                        crate::http_server::start_https_server(
                            listen_end_point,
                            app.clone(),
                            ssl_certificate,
                            None,
                            ssl_server_id,
                            endpoint_info,
                        );
                    }
                } else {
                    panic!(
                        "Certificate not found: {} for endpoint: {}",
                        ssl_id.as_str(),
                        listen_port
                    );
                }
            }
            settings::EndpointType::Https2 {
                ssl_id,
                client_ca_id,
                endpoint_info,
            } => {
                if let Some((cert, private_key)) = app
                    .settings_reader
                    .get_ssl_certificate(&ssl_id)
                    .await
                    .unwrap()
                {
                    ssl_server_id += 1;

                    let ssl_certificate = SslCertificate::new(
                        crate::flows::get_file(&cert).await,
                        crate::flows::get_file(&private_key).await,
                        private_key.as_str().as_str(),
                    );

                    if let Some(client_ca_id) = client_ca_id {
                        if let Some(client_cert) = app
                            .settings_reader
                            .get_client_certificate_ca(client_ca_id.as_str())
                            .await
                            .unwrap()
                        {
                            let client_ca = crate::flows::get_file(&client_cert).await;
                            crate::http_server::start_https2_server(
                                listen_end_point,
                                app.clone(),
                                ssl_certificate,
                                Some(client_ca.into()),
                                ssl_server_id,
                                endpoint_info,
                            );
                        } else {
                            panic!(
                                "Client certificate ca not found: {} for endpoint: {}",
                                client_ca_id.as_str(),
                                listen_port
                            );
                        }
                    } else {
                        crate::http_server::start_https2_server(
                            listen_end_point,
                            app.clone(),
                            ssl_certificate,
                            None,
                            ssl_server_id,
                            endpoint_info,
                        );
                    }
                } else {
                    panic!(
                        "Certificate not found: {} for endpoint: {}",
                        ssl_id.as_str(),
                        listen_port
                    );
                }
            }
            settings::EndpointType::Http2(endpoint_info) => {
                crate::http_server::start_h2_server(listen_end_point, app.clone(), endpoint_info);
            }
            settings::EndpointType::Tcp {
                remote_addr,
                debug,
                whitelisted_ip,
            } => {
                crate::tcp_port_forward::start_tcp(
                    app.clone(),
                    listen_end_point,
                    remote_addr,
                    whitelisted_ip,
                    debug,
                );
            }
            settings::EndpointType::TcpOverSsh {
                ssh_credentials,
                remote_host,
                debug,
            } => {
                crate::tcp_port_forward::start_tcp_over_ssh(
                    app.clone(),
                    listen_end_point,
                    ssh_credentials,
                    remote_host,
                    debug,
                );
            }
        }
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
