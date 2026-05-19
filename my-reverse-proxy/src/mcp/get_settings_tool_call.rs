use mcp_server_middleware::*;
use serde::*;

use crate::app::APP_CTX;
use crate::settings_compiled::SettingsCompiled;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct ConfigurationErrorEntry {
    #[property(description = "Host id whose configuration failed to apply")]
    pub host_id: String,

    #[property(description = "Error message recorded for the host")]
    pub error: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetSettingsInputData {}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct LocationSettingsSummary {
    #[property(description = "Location path prefix (e.g. '/')")]
    pub path: String,

    #[property(description = "Upstream / target (proxy_pass_to as written in YAML)")]
    pub proxy_pass_to: String,

    #[property(description = "Explicit location type from YAML (http/http2/https/static/drop/...), or empty if auto-detected from proxy_pass_to")]
    pub location_type: String,

    #[property(description = "Whitelisted IP list id for this location, if any")]
    pub whitelisted_ip: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct HostSettingsSummary {
    #[property(description = "Host id as written in YAML (e.g. 'mydomain.com:443' or 'unix:/var/run/x.sock')")]
    pub host_id: String,

    #[property(description = "Endpoint type: http / http2 / https / https2 / tcp / mcp")]
    pub endpoint_type: String,

    #[property(description = "True if debug logging is enabled on this endpoint")]
    pub debug: bool,

    #[property(description = "ssl_certificate id referenced by this host, if any")]
    pub ssl_certificate: String,

    #[property(description = "client_certificate_ca id referenced by this host, if any")]
    pub client_certificate_ca: String,

    #[property(description = "google_auth id referenced by this host, if any")]
    pub google_auth: String,

    #[property(description = "Endpoint template id, if any")]
    pub template_id: String,

    #[property(description = "Allowed users list id, if any")]
    pub allowed_users: String,

    #[property(description = "Whitelisted IP list id, if any")]
    pub whitelisted_ip: String,

    #[property(description = "True if HSTS is enabled on this endpoint")]
    pub hsts: bool,

    #[property(description = "Locations under this host")]
    pub locations: Vec<LocationSettingsSummary>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GlobalSettingsSummary {
    #[property(description = "HTTP control port from global_settings, if configured")]
    pub http_control_port: i64,

    #[property(description = "default_h2_livness_url from global_settings, if configured")]
    pub default_h2_livness_url: String,

    #[property(description = "show_error_description_on_error_page flag")]
    pub show_error_description_on_error_page: bool,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GatewayServerSummary {
    #[property(description = "Gateway server port")]
    pub port: i64,

    #[property(description = "True if debug logging is enabled on the gateway server")]
    pub debug: bool,

    #[property(description = "Count of authorized_keys paths configured")]
    pub authorized_keys_count: i64,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetSettingsResponse {
    #[property(description = "Resolved path to the settings file that was read")]
    pub settings_file: String,

    #[property(description = "Global settings block")]
    pub global: GlobalSettingsSummary,

    #[property(description = "All hosts defined in the settings file (and any included files)")]
    pub hosts: Vec<HostSettingsSummary>,

    #[property(description = "SSH config ids defined in settings")]
    pub ssh_ids: Vec<String>,

    #[property(description = "SSL certificate ids defined in settings")]
    pub ssl_certificate_ids: Vec<String>,

    #[property(description = "Client certificate CA ids defined in settings")]
    pub client_certificate_ca_ids: Vec<String>,

    #[property(description = "Google auth ids defined in settings")]
    pub g_auth_ids: Vec<String>,

    #[property(description = "Endpoint template ids defined in settings")]
    pub endpoint_template_ids: Vec<String>,

    #[property(description = "Allowed-users list ids defined in settings")]
    pub allowed_users_list_ids: Vec<String>,

    #[property(description = "IP white list ids defined in settings")]
    pub ip_white_list_ids: Vec<String>,

    #[property(description = "Gateway server block, if configured")]
    pub gateway_server: Option<GatewayServerSummary>,

    #[property(description = "Gateway client ids defined in settings")]
    pub gateway_client_ids: Vec<String>,

    #[property(description = "Errors recorded by the running configuration for endpoints that failed to apply. These are NOT settings-file errors — they come from the in-memory current_configuration state.")]
    pub current_configuration_errors: Vec<ConfigurationErrorEntry>,
}

pub struct GetSettingsHandler;

impl ToolDefinition for GetSettingsHandler {
    const FUNC_NAME: &'static str = "get_settings";
    const DESCRIPTION: &'static str = "Read and parse the settings YAML (~/.my-reverse-proxy, plus any `include:` files) and return a structured overview: hosts with their endpoint type and locations, global settings, ids of SSH/SSL/CA/g_auth/templates/users/ip-lists/gateway-clients, and any errors recorded for endpoints in the currently-running configuration. Use this to see what settings the file currently declares — call reload_settings afterwards to apply changes.";
}

#[async_trait::async_trait]
impl McpToolCall<GetSettingsInputData, GetSettingsResponse> for GetSettingsHandler {
    async fn execute_tool_call(
        &self,
        _model: GetSettingsInputData,
    ) -> Result<GetSettingsResponse, String> {
        let settings_file = resolve_settings_file_path();

        let settings = SettingsCompiled::load_settings().await?;

        let global = GlobalSettingsSummary {
            http_control_port: settings
                .get_http_control_port()
                .map(|p| p as i64)
                .unwrap_or(-1),
            default_h2_livness_url: settings.get_default_h2_livness_url().unwrap_or_default(),
            show_error_description_on_error_page: settings.get_show_error_description_on_error_page(),
        };

        let mut hosts: Vec<HostSettingsSummary> = settings
            .hosts
            .iter()
            .map(|(host_id, host)| {
                let locations = host
                    .locations
                    .iter()
                    .map(|loc| LocationSettingsSummary {
                        path: loc.path.clone().unwrap_or_default(),
                        proxy_pass_to: loc.proxy_pass_to.clone().unwrap_or_default(),
                        location_type: loc.location_type.clone().unwrap_or_default(),
                        whitelisted_ip: loc.whitelisted_ip.clone().unwrap_or_default(),
                    })
                    .collect();

                HostSettingsSummary {
                    host_id: host_id.clone(),
                    endpoint_type: host.endpoint.endpoint_type.clone(),
                    debug: host.endpoint.debug.unwrap_or(false),
                    ssl_certificate: host.endpoint.ssl_certificate.clone().unwrap_or_default(),
                    client_certificate_ca: host
                        .endpoint
                        .client_certificate_ca
                        .clone()
                        .unwrap_or_default(),
                    google_auth: host.endpoint.google_auth.clone().unwrap_or_default(),
                    template_id: host.endpoint.template_id.clone().unwrap_or_default(),
                    allowed_users: host.endpoint.allowed_users.clone().unwrap_or_default(),
                    whitelisted_ip: host.endpoint.whitelisted_ip.clone().unwrap_or_default(),
                    hsts: host.endpoint.hsts.unwrap_or(false),
                    locations,
                }
            })
            .collect();

        hosts.sort_by(|a, b| a.host_id.cmp(&b.host_id));

        let gateway_server = settings.gateway_server.as_ref().map(|g| GatewayServerSummary {
            port: g.port as i64,
            debug: g.is_debug(),
            authorized_keys_count: g.authorized_keys.len() as i64,
        });

        let current_configuration_errors = APP_CTX
            .current_configuration
            .get(|cfg| {
                cfg.error_configurations
                    .iter()
                    .map(|(host_id, error)| ConfigurationErrorEntry {
                        host_id: host_id.clone(),
                        error: error.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .await;

        Ok(GetSettingsResponse {
            settings_file,
            global,
            hosts,
            ssh_ids: sorted_keys(settings.ssh.keys()),
            ssl_certificate_ids: settings
                .ssl_certificates
                .iter()
                .map(|c| c.id.clone())
                .collect(),
            client_certificate_ca_ids: settings
                .client_certificate_ca
                .iter()
                .map(|c| c.id.clone())
                .collect(),
            g_auth_ids: sorted_keys(settings.g_auth.keys()),
            endpoint_template_ids: sorted_keys(settings.endpoint_templates.keys()),
            allowed_users_list_ids: sorted_keys(settings.allowed_users.keys()),
            ip_white_list_ids: sorted_keys(settings.ip_white_lists.keys()),
            gateway_server,
            gateway_client_ids: sorted_keys(settings.gateway_clients.keys()),
            current_configuration_errors,
        })
    }
}

fn resolve_settings_file_path() -> String {
    match std::env::var("HOME") {
        Ok(home) => format!("{}/.my-reverse-proxy", home),
        Err(_) => ".my-reverse-proxy".to_string(),
    }
}

fn sorted_keys<'a, I: IntoIterator<Item = &'a String>>(keys: I) -> Vec<String> {
    let mut out: Vec<String> = keys.into_iter().cloned().collect();
    out.sort();
    out
}
