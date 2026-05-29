use std::sync::Arc;

use crate::{
    configurations::*,
    settings::{EndpointTypeSettings, HostSettings},
    settings_compiled::SettingsCompiled,
};

/// Compiles a single host into a `ListenConfiguration`. For HTTP endpoints multiple hosts can
/// share one port, so `existing` is the configuration already accumulated for that port (from the
/// live config on a per-port reload, or from the map being built during a full reload) and the new
/// endpoint is merged into it. `None` means this is the first host on the port.
pub async fn compile_host_configuration(
    settings_model: &SettingsCompiled,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
    existing: Option<ListenConfiguration>,
) -> Result<ListenConfiguration, String> {
    match host_settings.endpoint.get_endpoint_type()? {
        EndpointTypeSettings::Http1 => {
            let http_endpoint_info = crate::scripts::compile_http_configuration(
                settings_model,
                host_endpoint,
                host_settings,
                ListenHttpEndpointType::Http1,
            )
            .await?;

            let config = super::merge_http_into_existing(existing, http_endpoint_info)?;

            return Ok(ListenConfiguration::Http(Arc::new(config)));
        }

        EndpointTypeSettings::Http2 => {
            let http_endpoint_info = crate::scripts::compile_http_configuration(
                settings_model,
                host_endpoint,
                host_settings,
                ListenHttpEndpointType::Http2,
            )
            .await?;

            let config = super::merge_http_into_existing(existing, http_endpoint_info)?;

            return Ok(ListenConfiguration::Http(Arc::new(config)));
        }

        EndpointTypeSettings::Https1 => {
            let http_endpoint_info = crate::scripts::compile_http_configuration(
                settings_model,
                host_endpoint,
                host_settings,
                ListenHttpEndpointType::Https1,
            )
            .await?;

            println!(
                "Merging Https1 configuration {}",
                http_endpoint_info.as_str()
            );

            let config = super::merge_http_into_existing(existing, http_endpoint_info)?;

            return Ok(ListenConfiguration::Http(Arc::new(config)));
        }

        EndpointTypeSettings::Https2 => {
            let http_endpoint_info = crate::scripts::compile_http_configuration(
                settings_model,
                host_endpoint,
                host_settings,
                ListenHttpEndpointType::Https2,
            )
            .await?;

            let config = super::merge_http_into_existing(existing, http_endpoint_info)?;

            return Ok(ListenConfiguration::Http(config.into()));
        }
        EndpointTypeSettings::Tcp => {
            let tcp_configuration =
                TcpEndpointHostConfig::new(settings_model, host_endpoint, host_settings).await?;

            return Ok(ListenConfiguration::Tcp(tcp_configuration.into()));
        }

        EndpointTypeSettings::Mcp => {
            let http_endpoint_info = crate::scripts::compile_http_configuration(
                settings_model,
                host_endpoint,
                host_settings,
                ListenHttpEndpointType::Mcp,
            )
            .await?;

            println!("Merging MCP configuration {}", http_endpoint_info.as_str());
            let config = super::merge_http_into_existing(existing, http_endpoint_info)?;

            return Ok(ListenConfiguration::Mcp(config.into()));
        }
    }
}
