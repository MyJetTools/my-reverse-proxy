use std::sync::Arc;

use crate::{
    configurations::{EndpointHttpHostString, HttpEndpointInfo, ListenHttpEndpointType},
    settings::HostSettings,
    settings_compiled::SettingsCompiled,
};

pub async fn compile_http_configuration(
    settings_model: &SettingsCompiled,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
    http_type: ListenHttpEndpointType,
) -> Result<HttpEndpointInfo, String> {
    let mut locations = Vec::with_capacity(host_settings.locations.len());

    let endpoint_whitelisted_ip =
        super::get_endpoint_white_listed_ip(settings_model, host_settings).await?;

    let allowed_user_list =
        crate::scripts::get_endpoint_users_list(settings_model, host_settings).await?;

    let modify_endpoints_headers =
        crate::scripts::get_endpoint_modify_headers(settings_model, host_settings);

    let (g_auth, ssl_cert_id, client_cert_ca) = if http_type.is_https_or_mcp() {
        let ssl_cert_id = super::make_sure_ssl_cert_exists(settings_model, host_settings).await?;

        let client_cert_ca =
            super::make_sure_client_ca_exists(settings_model, host_settings).await?;

        let g_auth = super::get_google_auth_credentials(settings_model, host_settings).await?;

        (g_auth, Some(ssl_cert_id), client_cert_ca)
    } else {
        (None, None, None)
    };

    for location_settings in &host_settings.locations {
        let proxy_pass_to =
            super::compile_location_proxy_pass_to(settings_model, location_settings).await?;
        {
            locations.push(Arc::new(proxy_pass_to));
        }
    }

    let http_endpoint_info = HttpEndpointInfo::new(
        host_endpoint,
        http_type,
        host_settings.endpoint.get_debug(),
        g_auth,
        ssl_cert_id,
        client_cert_ca,
        endpoint_whitelisted_ip,
        locations,
        allowed_user_list,
        modify_endpoints_headers,
    );

    Ok(http_endpoint_info)
}
