use std::sync::Arc;

use crate::{
    configurations::*,
    settings::{EndpointTypeSettings, HostSettings},
    settings_compiled::SettingsCompiled,
};

pub async fn compile_host_configuration(
    settings_model: &SettingsCompiled,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
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

            let config =
                super::merge_http_configuration_with_existing_port(http_endpoint_info).await?;

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

            let config =
                super::merge_http_configuration_with_existing_port(http_endpoint_info).await?;

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

            let config =
                super::merge_http_configuration_with_existing_port(http_endpoint_info).await?;

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

            let config =
                super::merge_http_configuration_with_existing_port(http_endpoint_info).await?;

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

            let config =
                super::merge_http_configuration_with_existing_port(http_endpoint_info).await?;

            return Ok(ListenConfiguration::Mpc(config.into()));
        }
    }
}
