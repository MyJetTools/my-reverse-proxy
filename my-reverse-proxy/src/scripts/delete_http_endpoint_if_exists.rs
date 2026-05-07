use crate::{configurations::ListenConfiguration, settings_compiled::SettingsCompiled};

pub async fn delete_http_endpoint_if_exists(
    settings: &SettingsCompiled,
    endpoint_id: &str,
) -> Result<(), String> {
    let host_endpoint = settings.get_endpoint_host_string(endpoint_id)?;
    let endpoint_port = host_endpoint.get_port();

    let listen_configuration = match &endpoint_port {
        crate::configurations::EndpointPort::Tcp(port) => {
            crate::app::APP_CTX
                .current_configuration
                .get(move |config| config.listen_tcp_endpoints.get(&port).cloned())
                .await
        }
        crate::configurations::EndpointPort::UnixSocket(unix_host) => {
            crate::app::APP_CTX
                .current_configuration
                .get(|config| config.listen_unix_socket_endpoints.get(unix_host).cloned())
                .await
        }
    };

    let Some(listen_configuration) = listen_configuration else {
        return Ok(());
    };

    if let ListenConfiguration::Http(http_config) = listen_configuration {
        if let Some(new_configuration) = http_config.delete_configuration(&host_endpoint) {
            if new_configuration.endpoints.is_empty() {
                crate::app::APP_CTX
                    .current_configuration
                    .write(move |config| match endpoint_port {
                        crate::configurations::EndpointPort::Tcp(port) => {
                            config.listen_tcp_endpoints.remove(&port);
                        }
                        crate::configurations::EndpointPort::UnixSocket(unix_host) => {
                            config.listen_unix_socket_endpoints.remove(&unix_host);
                        }
                    })
                    .await;
            } else {
                crate::app::APP_CTX
                    .current_configuration
                    .write(move |config| match endpoint_port {
                        crate::configurations::EndpointPort::Tcp(port) => {
                            config.listen_tcp_endpoints.insert(
                                port,
                                ListenConfiguration::Http(new_configuration.clone().into()),
                            );
                        }
                        crate::configurations::EndpointPort::UnixSocket(unix_host) => {
                            config.listen_unix_socket_endpoints.insert(
                                unix_host,
                                ListenConfiguration::Http(new_configuration.into()),
                            );
                        }
                    })
                    .await;
            }
        }
    }

    Ok(())
}
