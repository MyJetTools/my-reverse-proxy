use mcp_server_middleware::*;
use serde::*;

use crate::app::APP_CTX;

use super::ConfigurationErrorEntry;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct ReloadSettingsInputData {}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct ReloadSettingsResponse {
    #[property(description = "True if the settings YAML was read and applied without a top-level error. Per-host failures are still surfaced in `errors` even when this is true.")]
    pub ok: bool,

    #[property(description = "Top-level error string if the settings file could not be loaded/parsed (file missing, invalid YAML, variable resolution failure). Empty when ok=true.")]
    pub error: String,

    #[property(description = "Number of hosts in the running configuration after reload (TCP + Unix endpoints, summed across all listen endpoints).")]
    pub hosts_loaded: i64,

    #[property(description = "Errors recorded per host during apply. A non-empty list after a successful reload means some endpoints failed to apply but the rest were updated. Note: this is a snapshot of the running configuration's error map, which is NOT cleared between reloads.")]
    pub errors: Vec<ConfigurationErrorEntry>,
}

pub struct ReloadSettingsHandler;

impl ToolDefinition for ReloadSettingsHandler {
    const FUNC_NAME: &'static str = "reload_settings";
    const DESCRIPTION: &'static str = "Re-read the settings YAML (~/.my-reverse-proxy + includes) and apply it to the running proxy: re-syncs SSH config list, updates each host's configuration, and re-syncs TCP/Unix listeners. Upstream pools are preserved across reloads via id_string matching (see get_proxy_state_snapshot). Returns the running configuration's error map so per-host apply failures are visible.";
}

#[async_trait::async_trait]
impl McpToolCall<ReloadSettingsInputData, ReloadSettingsResponse> for ReloadSettingsHandler {
    async fn execute_tool_call(
        &self,
        _model: ReloadSettingsInputData,
    ) -> Result<ReloadSettingsResponse, String> {
        let (ok, error) = match crate::flows::load_everything_from_settings().await {
            Ok(()) => (true, String::new()),
            Err(err) => (false, err),
        };

        let (hosts_loaded, errors) = APP_CTX
            .current_configuration
            .get(|cfg| {
                let tcp_hosts: i64 = cfg
                    .listen_tcp_endpoints
                    .values()
                    .map(|listen| count_hosts(listen))
                    .sum();
                let unix_hosts: i64 = cfg
                    .listen_unix_socket_endpoints
                    .values()
                    .map(|listen| count_hosts(listen))
                    .sum();
                let errors: Vec<ConfigurationErrorEntry> = cfg
                    .error_configurations
                    .iter()
                    .map(|(host_id, error)| ConfigurationErrorEntry {
                        host_id: host_id.clone(),
                        error: error.clone(),
                    })
                    .collect();
                (tcp_hosts + unix_hosts, errors)
            })
            .await;

        Ok(ReloadSettingsResponse {
            ok,
            error,
            hosts_loaded,
            errors,
        })
    }
}

fn count_hosts(listen: &crate::configurations::ListenConfiguration) -> i64 {
    use crate::configurations::ListenConfiguration;
    match listen {
        ListenConfiguration::Http(http) | ListenConfiguration::Mcp(http) => {
            http.endpoints.len() as i64
        }
        ListenConfiguration::Tcp(_) => 1,
    }
}
