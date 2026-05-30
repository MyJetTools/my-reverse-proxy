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

    // Timeout cascade: HardCode < Global < Endpoint < Location.
    // Build the global→endpoint layer once, then layer each location on top.
    let global_timeouts = settings_model.get_global_timeouts();
    let endpoint_timeouts = global_timeouts.overriden_by(&host_settings.endpoint.timeouts);

    // MCP timeouts are tunnel-scoped: resolved from the endpoint layer, with the
    // first location allowed to override (an mcp endpoint carries one location).
    let mut mcp_resolved = endpoint_timeouts.resolve();

    let listen_host = host_endpoint.as_str().to_string();
    for (location_index, location_settings) in host_settings.locations.iter().enumerate() {
        let resolved = endpoint_timeouts
            .overriden_by(&location_settings.timeouts)
            .resolve();

        if location_index == 0 {
            mcp_resolved = resolved;
        }

        let proxy_pass_to = super::compile_location_proxy_pass_to(
            settings_model,
            location_settings,
            &listen_host,
            &resolved,
        )
        .await?;
        {
            locations.push(Arc::new(proxy_pass_to));
        }
    }

    let mcp_settings = crate::configurations::McpEndpointSettings::new(
        mcp_resolved.read_timeout,
        mcp_resolved.write_timeout,
        host_settings.endpoint.get_mcp_buffer_size(),
    );

    // Endpoint-scoped transport read/write timeouts (global → endpoint), applied
    // to every byte pump of this endpoint's connections.
    let endpoint_resolved = endpoint_timeouts.resolve();
    let http_timeouts = crate::types::HttpTimeouts {
        read_timeout: endpoint_resolved.read_timeout,
        write_timeout: endpoint_resolved.write_timeout,
    };

    let http_endpoint_info = HttpEndpointInfo::new(
        host_endpoint,
        http_type,
        host_settings.endpoint.get_debug(),
        host_settings.endpoint.get_inject_country(),
        g_auth,
        ssl_cert_id,
        client_cert_ca,
        endpoint_whitelisted_ip,
        locations,
        allowed_user_list,
        modify_endpoints_headers,
        host_settings.endpoint.keep_alive.unwrap_or(true),
        host_settings
            .endpoint
            .track_metrics_by_all_domains
            .unwrap_or(false),
        host_settings.endpoint.hsts.unwrap_or(false),
        mcp_settings,
        http_timeouts,
    );

    Ok(http_endpoint_info)
}
