use crate::{
    configurations::{
        EndpointHttpHostString, ListenConfiguration, MyReverseProxyRemoteEndpoint,
        TcpEndpointHostConfig,
    },
    settings::HostSettings,
    settings_compiled::SettingsCompiled,
};

pub async fn compile_tcp_configuration(
    settings_model: &SettingsCompiled,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
) -> Result<ListenConfiguration, String> {
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
        super::get_endpoint_white_listed_ip(settings_model, host_settings).await?;

    let remote_host =
        MyReverseProxyRemoteEndpoint::try_parse(remote_host.as_str(), settings_model).await?;

    let result = TcpEndpointHostConfig {
        host_endpoint,
        remote_host: remote_host.into(),
        debug: host_settings.endpoint.get_debug(),
        ip_white_list_id,
    };

    Ok(ListenConfiguration::Tcp(result.into()))
}
