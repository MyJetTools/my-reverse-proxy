use mcp_server_middleware::*;
use serde::*;

use crate::app::APP_CTX;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetProxyLogsInputData {
    #[property(
        description = "Listening port id: TCP port number as a decimal string (e.g. '443') or a unix socket path. Returns pre-endpoint logs (e.g. rejected/unresolved connections). Optional."
    )]
    pub port: Option<String>,

    #[property(
        description = "Endpoint host string exactly as configured, e.g. 'trade-dev.x-fine.online:443'. Returns endpoint-level logs — this is where every 5xx returned to a client is recorded (both proxy-generated 503/504/500 with the upstream failure reason, and 5xx passed through from the upstream). Optional."
    )]
    pub endpoint: Option<String>,

    #[property(
        description = "location_id (same id as in get_proxy_state_snapshot / lookup_pool). Returns location-level logs — upstream request failures are always recorded here with the error detail. Optional."
    )]
    pub location_id: Option<i64>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct ProxyLogEntryModel {
    #[property(description = "Which buffer the entry came from: 'port:<id>' / 'endpoint:<host>' / 'location:<id>'")]
    pub scope: String,

    #[property(description = "Event time, RFC-3339")]
    pub moment: String,

    #[property(description = "Source client IP, empty if not resolvable (e.g. unix socket)")]
    pub ip: String,

    #[property(description = "Log message")]
    pub message: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetProxyLogsResponse {
    #[property(description = "Matching log entries, newest first, merged across the requested scopes. Each scope keeps at most the last 100 messages in memory.")]
    pub entries: Vec<ProxyLogEntryModel>,
}

pub struct GetProxyLogsHandler;

impl ToolDefinition for GetProxyLogsHandler {
    const FUNC_NAME: &'static str = "get_proxy_logs";
    const DESCRIPTION: &'static str = "Read the in-memory proxy logs by port, endpoint host and/or location_id (at least one filter required; ring buffers of 100 entries each). Every 5xx returned to a client is ALWAYS recorded at the endpoint scope — proxy-generated ones ('Returned 503 to client … upstream failure: …') and upstream-originated ones ('Upstream responded 500 …') — and upstream request failures are always recorded at the location scope. Use this to answer 'why did clients get 50x' without restarting or enabling debug mode; verbose request/response traces additionally appear when debug is enabled for the endpoint/location.";
}

#[async_trait::async_trait]
impl McpToolCall<GetProxyLogsInputData, GetProxyLogsResponse> for GetProxyLogsHandler {
    async fn execute_tool_call(
        &self,
        model: GetProxyLogsInputData,
    ) -> Result<GetProxyLogsResponse, String> {
        if model.port.is_none() && model.endpoint.is_none() && model.location_id.is_none() {
            return Err("Provide at least one of: port, endpoint, location_id".to_string());
        }

        let mut raw: Vec<(i64, ProxyLogEntryModel)> = Vec::new();

        if let Some(port) = model.port.as_deref() {
            absorb(&mut raw, APP_CTX.proxy_logs.get_by_port(port), format!("port:{}", port));
        }
        if let Some(endpoint) = model.endpoint.as_deref() {
            absorb(
                &mut raw,
                APP_CTX.proxy_logs.get_by_endpoint(endpoint),
                format!("endpoint:{}", endpoint),
            );
        }
        if let Some(location_id) = model.location_id {
            absorb(
                &mut raw,
                APP_CTX.proxy_logs.get_by_location(location_id),
                format!("location:{}", location_id),
            );
        }

        // Newest first across all requested scopes.
        raw.sort_by(|a, b| b.0.cmp(&a.0));

        Ok(GetProxyLogsResponse {
            entries: raw.into_iter().map(|(_, entry)| entry).collect(),
        })
    }
}

fn absorb(
    out: &mut Vec<(i64, ProxyLogEntryModel)>,
    entries: Vec<crate::app::ProxyLogEntry>,
    scope: String,
) {
    for entry in entries {
        out.push((
            entry.moment.unix_microseconds,
            ProxyLogEntryModel {
                scope: scope.clone(),
                moment: entry.moment.to_rfc3339(),
                ip: entry.ip.unwrap_or_default(),
                message: entry.message,
            },
        ));
    }
}
