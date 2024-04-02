use std::sync::Arc;

use crate::app::AppContext;

pub async fn kick_off_endpoints(app: &Arc<AppContext>) {
    let app_configuration = app.current_app_configuration.read().await;
    for (listen_port, listen_port_configuration) in
        &app_configuration.as_ref().unwrap().listen_ports
    {
        let listen_end_point = std::net::SocketAddr::new([0, 0, 0, 0].into(), *listen_port);

        match listen_port_configuration {
            crate::app_configuration::ListenPortConfiguration::Http(port_configuration) => {
                if port_configuration.is_https() {
                    crate::http_server::start_https_server(listen_end_point, app.clone());
                } else if port_configuration.is_http1() {
                    crate::http_server::start_http_server(listen_end_point, app.clone());
                } else {
                    crate::http_server::start_h2_server(listen_end_point, app.clone());
                }
            }
            crate::app_configuration::ListenPortConfiguration::Tcp(endpoint_info) => {
                crate::tcp_port_forward::start_tcp(
                    app.clone(),
                    listen_end_point,
                    endpoint_info.clone(),
                );
            }
            crate::app_configuration::ListenPortConfiguration::TcpOverSsh(endpoint_info) => {
                crate::tcp_port_forward::start_tcp_over_ssh(
                    app.clone(),
                    listen_end_point,
                    endpoint_info.clone(),
                );
            } /*
              EndpointType::Http(http_endpoint_info) => match &http_endpoint_info.http_type {
                  crate::app_configuration::HttpType::Http1 => {
                      crate::http_server::start_http_server(
                          listen_end_point,
                          app.clone(),
                          host_config.connection_info.clone(),
                      );
                  }
                  crate::app_configuration::HttpType::Https1 => {
                      crate::http_server::start_https_server(
                          listen_end_point,
                          app.clone(),
                          ssl_certificate,
                      );
                  }
                  crate::app_configuration::HttpType::Http2 => {
                      crate::http_server::start_http_server(
                          listen_end_point,
                          app.clone(),
                          host_config.connection_info.clone(),
                      );
                  }
                  crate::app_configuration::HttpType::Https2 => {
                      crate::http_server::start_https_server(
                          listen_end_point,
                          app.clone(),
                          ssl_certificate,
                      );
                  }
              },
              EndpointType::Https { ssl_id, .. } => {
                  if let Some((cert, private_key)) = app
                      .settings_reader
                      .get_ssl_certificate(&ssl_id)
                      .await
                      .unwrap()
                  {
                      let ssl_certificate = SslCertificate::new(
                          crate::flows::get_file(&cert).await,
                          crate::flows::get_file(&private_key).await,
                          private_key.as_str().as_str(),
                      );

                      crate::http_server::start_https_server(
                          listen_end_point,
                          app.clone(),
                          ssl_certificate,
                      );
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

              EndpointType::Http2(host_config) => {
                  crate::http_server::start_h2_server(
                      listen_end_point,
                      app.clone(),
                      host_config.connection_info.clone(),
                  );
              }
              EndpointType::Tcp(endpoint_info) => {
                  crate::tcp_port_forward::start_tcp(
                      app.clone(),
                      listen_end_point,
                      endpoint_info.clone(),
                  );
              }
              EndpointType::TcpOverSsh(endpoint_info) => {
                  crate::tcp_port_forward::start_tcp_over_ssh(
                      app.clone(),
                      listen_end_point,
                      endpoint_info.clone(),
                  );
              }
                 */
        }
    }
}
