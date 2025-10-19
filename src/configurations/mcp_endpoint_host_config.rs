use crate::{settings::HostSettings, settings_compiled::SettingsCompiled};

use super::*;

pub struct McpEndpointHostConfig {
    pub host_endpoint: EndpointHttpHostString,
    pub debug: bool,
    pub remote_host: MyReverseProxyRemoteEndpoint,
}

impl McpEndpointHostConfig {
    pub async fn new(
        settings_model: &SettingsCompiled,
        host_endpoint: EndpointHttpHostString,
        host_settings: &HostSettings,
    ) -> Result<Self, String> {
        let remote_host = if let Some(location_settings) = host_settings.locations.first() {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for tcp location type".to_string());
            }

            location_settings.proxy_pass_to.as_ref().unwrap()
        } else {
            return Err(format!(
                "No location found for tcp host {}",
                host_endpoint.as_str()
            ));
        };

        let remote_host =
            MyReverseProxyRemoteEndpoint::try_parse(remote_host.as_str(), settings_model).await?;

        let result = Self {
            host_endpoint,
            debug: host_settings.endpoint.get_debug(),
            remote_host,
        };

        Ok(result)
    }
}
