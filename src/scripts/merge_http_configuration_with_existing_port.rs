use std::sync::Arc;

use crate::configurations::*;

pub async fn merge_http_configuration_with_existing_port(
    http_endpoint_info: HttpEndpointInfo,
) -> Result<HttpListenPortConfiguration, String> {
    let endpoint_port = http_endpoint_info.host_endpoint.get_port();
    let host_key = http_endpoint_info.host_endpoint.as_str().to_string();

    let listen_config = match endpoint_port {
        EndpointPort::Tcp(port) => {
            crate::app::APP_CTX
                .current_configuration
                .get(move |config| config.listen_tcp_endpoints.get(&port).cloned())
                .await
        }
        EndpointPort::UnixSocket(unix_host) => {
            crate::app::APP_CTX
                .current_configuration
                .get(move |config| config.listen_unix_socket_endpoints.get(&unix_host).cloned())
                .await
        }
    };

    let listen_host = http_endpoint_info.host_endpoint.get_listen_host();

    let Some(configuration) = listen_config else {
        return Ok(HttpListenPortConfiguration::new(
            Arc::new(http_endpoint_info),
            listen_host,
        ));
    };

    match configuration {
        ListenConfiguration::Http(config) => {
            check_endpoint_type(&config, &http_endpoint_info)?;
            let mut config = config.as_ref().clone();
            config.insert_or_replace_configuration(http_endpoint_info);
            return Ok(config);
        }
        ListenConfiguration::Tcp(_) => {
            return Err(format!(
                "Can not apply endpoint {}. Endpoint {} is already configured as TCP.",
                http_endpoint_info.host_endpoint.as_str(),
                host_key
            ));
        }
        ListenConfiguration::Mpc(config) => {
            check_endpoint_type(&config, &http_endpoint_info)?;
            let mut config = config.as_ref().clone();
            config.insert_or_replace_configuration(http_endpoint_info);
            return Ok(config);
        }
    }
}

fn check_endpoint_type(
    config: &HttpListenPortConfiguration,
    http_endpoint: &HttpEndpointInfo,
) -> Result<(), String> {
    if config
        .listen_endpoint_type
        .can_be_under_the_same_port(http_endpoint.listen_endpoint_type)
    {
        return Ok(());
    }

    return Err(format!(
            "Can not apply endpoint {} which has {:?} type to the endpoint {} already configured as {:?}.",
            http_endpoint.host_endpoint.as_str(),
            http_endpoint.listen_endpoint_type,
            http_endpoint.host_endpoint.as_str(),
            config.listen_endpoint_type
        ));
}
