use mcp_server_middleware::*;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::*;

use crate::app::APP_CTX;

use super::{build_h1_pool_snapshot, build_h2_pool_snapshot, PoolSnapshot};

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct LookupPoolInputData {
    #[property(
        description = "location_id of the location to look up. Optional — if omitted, id_string is used instead."
    )]
    pub location_id: Option<i64>,

    #[property(
        description = "id_string to look up (e.g. '0.0.0.0:443|/->http2|http://upstream:8080'). Optional — if omitted, location_id is used instead."
    )]
    pub id_string: Option<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct LookupPoolResponse {
    #[property(description = "Snapshot capture time, RFC-3339")]
    pub captured_at: String,

    #[property(
        description = "Pools matching the lookup. Normally 0 or 1 — but if the same id_string somehow exists in multiple registries (regression), all matches are returned."
    )]
    pub matches: Vec<PoolSnapshot>,
}

pub struct LookupPoolHandler;

impl ToolDefinition for LookupPoolHandler {
    const FUNC_NAME: &'static str = "lookup_pool";
    const DESCRIPTION: &'static str = "Look up an upstream pool by location_id and/or id_string across all 6 pool registries. Returns the same per-pool detail as get_proxy_state_snapshot (last_status / live_disposables / per-entry dead / rented / last_success) but only for matching pools — useful for re-checking one upstream right after observing a state change, e.g. to confirm whether the proxy now sees it as 'ok' or still 'error'.";
}

#[async_trait::async_trait]
impl McpToolCall<LookupPoolInputData, LookupPoolResponse> for LookupPoolHandler {
    async fn execute_tool_call(
        &self,
        model: LookupPoolInputData,
    ) -> Result<LookupPoolResponse, String> {
        if model.location_id.is_none() && model.id_string.is_none() {
            return Err("Either location_id or id_string must be provided".to_string());
        }

        let now = DateTimeAsMicroseconds::now();
        let mut matches: Vec<PoolSnapshot> = Vec::new();

        let id_match = |loc_id: i64, id_str: &str| -> bool {
            if let Some(target_id) = model.location_id {
                if target_id == loc_id {
                    return true;
                }
            }
            if let Some(target_str) = model.id_string.as_deref() {
                if target_str == id_str {
                    return true;
                }
            }
            false
        };

        matches.extend(filter_h1(&APP_CTX.h1_tcp_pools, "h1_tcp", now, &id_match));
        matches.extend(filter_h1(&APP_CTX.h1_tls_pools, "h1_tls", now, &id_match));
        matches.extend(filter_h1(&APP_CTX.h1_uds_pools, "h1_uds", now, &id_match));
        matches.extend(filter_h2(&APP_CTX.h2_tcp_pools, "h2_tcp", now, &id_match));
        matches.extend(filter_h2(&APP_CTX.h2_tls_pools, "h2_tls", now, &id_match));
        matches.extend(filter_h2(&APP_CTX.h2_uds_pools, "h2_uds", now, &id_match));

        Ok(LookupPoolResponse {
            captured_at: now.to_rfc3339(),
            matches,
        })
    }
}

fn filter_h1<TStream, TConnector>(
    reg: &crate::upstream_h1_pool::H1PoolRegistry<TStream, TConnector>,
    registry: &str,
    now: DateTimeAsMicroseconds,
    id_match: &impl Fn(i64, &str) -> bool,
) -> Vec<PoolSnapshot>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: my_http_client::MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    reg.list_pools()
        .iter()
        .filter(|pool| id_match(pool.desc.location_id, &pool.desc.id_string))
        .map(|pool| build_h1_pool_snapshot(pool, registry, now))
        .collect()
}

fn filter_h2<TStream, TConnector>(
    reg: &crate::upstream_h2_pool::H2PoolRegistry<TStream, TConnector>,
    registry: &str,
    now: DateTimeAsMicroseconds,
    id_match: &impl Fn(i64, &str) -> bool,
) -> Vec<PoolSnapshot>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: my_http_client::MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    reg.list_pools()
        .iter()
        .filter(|pool| id_match(pool.desc.location_id, &pool.desc.id_string))
        .map(|pool| build_h2_pool_snapshot(pool, registry, now))
        .collect()
}
