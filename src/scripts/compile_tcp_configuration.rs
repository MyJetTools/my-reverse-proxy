use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{
    app::AppContext,
    configurations::{EndpointHttpHostString, ListenConfiguration, TcpEndpointHostConfig},
    settings::{HostSettings, SettingsModel},
};

pub async fn compile_tcp_configuration(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
) -> Result<ListenConfiguration, String> {
    let remote_host = if let Some(location_settings) = host_settings.locations.first() {
        if location_settings.proxy_pass_to.is_none() {
            return Err("proxy_pass_to is required for tcp location type".to_string());
        }
        let proxy_pass_to = super::apply_variables(
            settings_model,
            location_settings.proxy_pass_to.as_ref().unwrap(),
        )?;

        proxy_pass_to.to_string()
    } else {
        return Err(format!(
            "No location found for tcp host {}",
            host_endpoint.as_str()
        ));
    };

    let ip_white_list_id =
        super::get_endpoint_white_listed_ip(app, settings_model, host_settings).await?;

    let over_ssh_connection = OverSshConnectionSettings::try_parse(remote_host.as_str());

    if over_ssh_connection.is_none() {
        return Err(format!("Invalid remote host {}", remote_host));
    }

    let over_ssh_connection = super::ssh::enrich_with_private_key_or_password(
        over_ssh_connection.unwrap(),
        settings_model,
    )
    .await?;

    let result = TcpEndpointHostConfig {
        host_endpoint,
        remote_host: over_ssh_connection.into(),
        debug: host_settings.endpoint.get_debug(),
        ip_white_list_id,
    };

    Ok(ListenConfiguration::Tcp(result.into()))
}
