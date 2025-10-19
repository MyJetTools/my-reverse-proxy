use std::sync::Arc;

use crate::configurations::*;

pub async fn merge_http_configuration_with_existing_port(
    http_endpoint_info: HttpEndpointInfo,
) -> Result<HttpListenPortConfiguration, String> {
    let port = http_endpoint_info.host_endpoint.get_port();

    let configuration = crate::app::APP_CTX
        .current_configuration
        .get(move |config| config.listen_endpoints.get(&port).cloned())
        .await;

    if configuration.is_none() {
        return Ok(HttpListenPortConfiguration::new(Arc::new(
            http_endpoint_info,
        )));
    }

    let configuration = configuration.unwrap();

    match configuration {
        ListenConfiguration::Http(config) => {
            check_endpoint_type(&config, &http_endpoint_info)?;
            let mut config = config.as_ref().clone();
            config.insert_or_replace_configuration(http_endpoint_info);
            return Ok(config);
        }
        ListenConfiguration::Tcp(_) => {
            return Err(format!(
                "Can not apply endpoint {}. Port {} is already configured as TCP.",
                http_endpoint_info.host_endpoint.as_str(),
                port
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
            "Can not apply endpoint {} which has {:?} type to the port {} is already configured as {:?}.",
            http_endpoint.host_endpoint.as_str(),
            http_endpoint.listen_endpoint_type,
            http_endpoint.host_endpoint.get_port(),
            config.listen_endpoint_type
        ));
}
