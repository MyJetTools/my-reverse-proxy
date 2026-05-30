use std::sync::Arc;

use crate::{settings::HostSettings, settings_compiled::SettingsCompiled};

use super::*;

pub struct TcpEndpointHostConfig {
    pub host_endpoint: EndpointHttpHostString,
    pub remote_host: Arc<MyReverseProxyRemoteEndpoint>,
    pub debug: bool,
    pub ip_white_list_id: Option<String>,
    /// Transport read/write idle timeouts (resolved cascade: global → endpoint).
    pub timeouts: crate::types::HttpTimeouts,
}

impl TcpEndpointHostConfig {
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

        let ip_white_list_id =
            crate::scripts::get_endpoint_white_listed_ip(settings_model, host_settings).await?;

        let remote_host =
            MyReverseProxyRemoteEndpoint::try_parse(remote_host.as_str(), settings_model).await?;

        // Transport timeout cascade for this tcp endpoint: global → endpoint.
        let resolved = settings_model
            .get_global_timeouts()
            .overriden_by(&host_settings.endpoint.timeouts)
            .resolve();
        let timeouts = crate::types::HttpTimeouts {
            read_timeout: resolved.read_timeout,
            write_timeout: resolved.write_timeout,
        };

        let result = Self {
            host_endpoint,
            remote_host: remote_host.into(),
            debug: host_settings.endpoint.get_debug(),
            ip_white_list_id,
            timeouts,
        };

        Ok(result)
    }
}
