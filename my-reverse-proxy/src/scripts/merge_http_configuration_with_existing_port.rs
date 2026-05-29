use std::sync::Arc;

use crate::configurations::*;

/// Pure merge: given the existing `ListenConfiguration` for a port (if any) and a new HTTP
/// endpoint, produce the combined port configuration. Does NOT touch global state — the caller
/// supplies `existing` from wherever it accumulates ports: the live configuration (targeted
/// per-port reload) or a fresh map being built for a full reload + atomic swap.
pub fn merge_http_into_existing(
    existing: Option<ListenConfiguration>,
    http_endpoint_info: HttpEndpointInfo,
) -> Result<HttpListenPortConfiguration, String> {
    let listen_host = http_endpoint_info.host_endpoint.get_listen_host();

    let Some(configuration) = existing else {
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
            Ok(config)
        }
        ListenConfiguration::Tcp(_) => Err(format!(
            "Can not apply endpoint {}. It is already configured as TCP.",
            http_endpoint_info.host_endpoint.as_str(),
        )),
        ListenConfiguration::Mcp(config) => {
            check_endpoint_type(&config, &http_endpoint_info)?;
            let mut config = config.as_ref().clone();
            config.insert_or_replace_configuration(http_endpoint_info);
            Ok(config)
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
