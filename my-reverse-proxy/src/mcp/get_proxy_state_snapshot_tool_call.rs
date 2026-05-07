use std::sync::atomic::Ordering;

use ahash::AHashSet;
use mcp_server_middleware::*;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::*;

use crate::app::APP_CTX;
use crate::configurations::{AppConfigurationInner, ListenConfiguration};

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetProxyStateSnapshotInputData {}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct EntrySnapshot {
    #[property(description = "Index of this entry within the pool's clients vec")]
    pub index: i64,

    #[property(
        description = "True if the entry is currently marked dead — supervisor / get_connection will spawn a revive"
    )]
    pub dead: bool,

    #[property(
        description = "RFC-3339 timestamp of the last successful do_request on this entry"
    )]
    pub last_success: String,

    #[property(description = "Seconds since last_success at snapshot time")]
    pub idle_secs: i64,

    #[property(
        description = "h1: whether the entry is currently rented (in-flight request). null for h2 entries which have no rented flag"
    )]
    pub rented: Option<bool>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct PoolSnapshot {
    #[property(
        description = "Registry that owns this pool: h1_tcp / h1_tls / h1_uds / h2_tcp / h2_tls / h2_uds"
    )]
    pub registry: String,

    #[property(description = "location_id that the pool is keyed by in its registry")]
    pub location_id: i64,

    #[property(
        description = "Human-readable pool name used in Prometheus metrics and admin contracts (e.g. h1://host:port#42)"
    )]
    pub pool_name: String,

    #[property(
        description = "Logical identity string used by find_location_id_by_id_string to preserve pools across config reloads. Format: '{listen_host}|{path}->{type}|{remote_host}'"
    )]
    pub id_string: String,

    #[property(description = "Number of entries currently NOT marked dead")]
    pub alive_count: i64,

    #[property(description = "Total number of entries in the pool's clients vec")]
    pub total_count: i64,

    #[property(
        description = "Whether the pool has been signaled to shut down. drain_unused removes shutdown pools, so visible pools should normally have shutdown=false"
    )]
    pub shutdown: bool,

    #[property(description = "Per-entry detail")]
    pub entries: Vec<EntrySnapshot>,

    #[property(
        description = "True if a location with this location_id exists in the current configuration. False = orphan pool — drain_unused will remove it on the next GcPoolsTimer tick (≤60s)."
    )]
    pub has_matching_location: bool,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct LocationSnapshot {
    #[property(description = "Listener label: 'tcp:<port>' or 'unix:<socket_path>'")]
    pub listen: String,

    #[property(description = "Endpoint host (e.g. 'example.com:443')")]
    pub endpoint_host: String,

    #[property(description = "Location path prefix")]
    pub path: String,

    #[property(description = "Per-process location_id used as the pool registry key")]
    pub location_id: i64,

    #[property(
        description = "Logical identity string. The same id_string between two compilations should preserve location_id via find_location_id_by_id_string."
    )]
    pub id_string: String,

    #[property(
        description = "proxy_pass_to type: http1 / http2 / mcp / unix+http1 / unix+http2 / files_path / static / drop"
    )]
    pub proxy_pass_to_type: String,

    #[property(description = "Stringified upstream target (host:port, file path, or static description)")]
    pub proxy_pass_to: String,

    #[property(
        description = "True if a pool with this location_id is registered in any of the 6 pool registries. False is normal until the first request creates the pool lazily — but is suspicious if the same location had a pool earlier."
    )]
    pub has_pool: bool,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetProxyStateSnapshotResponse {
    #[property(description = "Snapshot capture time, RFC-3339")]
    pub captured_at: String,

    #[property(description = "All pools currently registered across the 6 pool registries")]
    pub pools: Vec<PoolSnapshot>,

    #[property(description = "All locations currently in the active configuration")]
    pub locations: Vec<LocationSnapshot>,

    #[property(description = "location_ids that exist in pools but NOT in current_configuration — orphaned pools that will be drained")]
    pub orphan_pool_location_ids: Vec<i64>,

    #[property(description = "location_ids that exist in current_configuration but have NO pool yet (lazy creation pending or pool was drained)")]
    pub naked_location_ids: Vec<i64>,
}

pub struct GetProxyStateSnapshotHandler;

impl ToolDefinition for GetProxyStateSnapshotHandler {
    const FUNC_NAME: &'static str = "get_proxy_state_snapshot";
    const DESCRIPTION: &'static str = "Detailed snapshot of all upstream pools across the 6 pool registries (h1/h2 × tcp/tls/uds) and all locations from the current configuration. Includes per-entry pool state (dead / last_success / idle_secs / rented), pool alive/total counts, shutdown flag, location id_string, and orphan/naked correlation flags. Use to diagnose pool disappearance, location_id drift across reloads, and connection lifecycle issues.";
}

#[async_trait::async_trait]
impl McpToolCall<GetProxyStateSnapshotInputData, GetProxyStateSnapshotResponse>
    for GetProxyStateSnapshotHandler
{
    async fn execute_tool_call(
        &self,
        _model: GetProxyStateSnapshotInputData,
    ) -> Result<GetProxyStateSnapshotResponse, String> {
        let now = DateTimeAsMicroseconds::now();

        let mut pools: Vec<PoolSnapshot> = Vec::new();
        pools.extend(snapshot_h1_pools(&APP_CTX.h1_tcp_pools, "h1_tcp", now));
        pools.extend(snapshot_h1_pools(&APP_CTX.h1_tls_pools, "h1_tls", now));
        pools.extend(snapshot_h1_pools(&APP_CTX.h1_uds_pools, "h1_uds", now));
        pools.extend(snapshot_h2_pools(&APP_CTX.h2_tcp_pools, "h2_tcp", now));
        pools.extend(snapshot_h2_pools(&APP_CTX.h2_tls_pools, "h2_tls", now));
        pools.extend(snapshot_h2_pools(&APP_CTX.h2_uds_pools, "h2_uds", now));

        let mut locations = APP_CTX
            .current_configuration
            .get(|cfg| collect_locations(cfg))
            .await;

        let location_id_set: AHashSet<i64> = locations.iter().map(|l| l.location_id).collect();
        let pool_id_set: AHashSet<i64> = pools.iter().map(|p| p.location_id).collect();

        for pool in pools.iter_mut() {
            pool.has_matching_location = location_id_set.contains(&pool.location_id);
        }
        for loc in locations.iter_mut() {
            loc.has_pool = pool_id_set.contains(&loc.location_id);
        }

        let orphan_pool_location_ids: Vec<i64> = pools
            .iter()
            .filter(|p| !p.has_matching_location)
            .map(|p| p.location_id)
            .collect();
        let naked_location_ids: Vec<i64> = locations
            .iter()
            .filter(|l| !l.has_pool)
            .map(|l| l.location_id)
            .collect();

        Ok(GetProxyStateSnapshotResponse {
            captured_at: now.to_rfc3339(),
            pools,
            locations,
            orphan_pool_location_ids,
            naked_location_ids,
        })
    }
}

fn snapshot_h1_pools<TStream, TConnector>(
    reg: &crate::upstream_h1_pool::H1PoolRegistry<TStream, TConnector>,
    registry: &str,
    now: DateTimeAsMicroseconds,
) -> Vec<PoolSnapshot>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: my_http_client::MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    reg.list_pools()
        .iter()
        .map(|pool| {
            let entries_snap = pool.clients.load();
            let entries: Vec<EntrySnapshot> = entries_snap
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let last = entry.last_success.as_date_time();
                    let idle_secs = now
                        .duration_since(last)
                        .as_positive_or_zero()
                        .as_secs() as i64;
                    EntrySnapshot {
                        index: i as i64,
                        dead: entry.dead.load(Ordering::Relaxed),
                        last_success: last.to_rfc3339(),
                        idle_secs,
                        rented: Some(entry.rented.load(Ordering::Relaxed)),
                    }
                })
                .collect();

            PoolSnapshot {
                registry: registry.to_string(),
                location_id: pool.desc.location_id,
                pool_name: pool.desc.name.clone(),
                id_string: pool.desc.id_string.clone(),
                alive_count: pool.alive_count() as i64,
                total_count: pool.total_count() as i64,
                shutdown: pool.shutdown.load(Ordering::Relaxed),
                entries,
                has_matching_location: false,
            }
        })
        .collect()
}

fn snapshot_h2_pools<TStream, TConnector>(
    reg: &crate::upstream_h2_pool::H2PoolRegistry<TStream, TConnector>,
    registry: &str,
    now: DateTimeAsMicroseconds,
) -> Vec<PoolSnapshot>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: my_http_client::MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    reg.list_pools()
        .iter()
        .map(|pool| {
            let entries_snap = pool.clients.load();
            let entries: Vec<EntrySnapshot> = entries_snap
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let last = entry.last_success.as_date_time();
                    let idle_secs = now
                        .duration_since(last)
                        .as_positive_or_zero()
                        .as_secs() as i64;
                    EntrySnapshot {
                        index: i as i64,
                        dead: entry.dead.load(Ordering::Relaxed),
                        last_success: last.to_rfc3339(),
                        idle_secs,
                        rented: None,
                    }
                })
                .collect();

            PoolSnapshot {
                registry: registry.to_string(),
                location_id: pool.desc.location_id,
                pool_name: pool.desc.name.clone(),
                id_string: pool.desc.id_string.clone(),
                alive_count: pool.alive_count() as i64,
                total_count: pool.total_count() as i64,
                shutdown: pool.shutdown.load(Ordering::Relaxed),
                entries,
                has_matching_location: false,
            }
        })
        .collect()
}

fn collect_locations(cfg: &AppConfigurationInner) -> Vec<LocationSnapshot> {
    let mut out = Vec::new();
    for (port, listen) in &cfg.listen_tcp_endpoints {
        let listen_label = format!("tcp:{}", port);
        absorb_listen(listen, &listen_label, &mut out);
    }
    for (path, listen) in &cfg.listen_unix_socket_endpoints {
        let listen_label = format!("unix:{}", path.as_str());
        absorb_listen(listen, &listen_label, &mut out);
    }
    out
}

fn absorb_listen(
    listen: &ListenConfiguration,
    listen_label: &str,
    out: &mut Vec<LocationSnapshot>,
) {
    let endpoints = match listen {
        ListenConfiguration::Http(http) | ListenConfiguration::Mcp(http) => &http.endpoints,
        ListenConfiguration::Tcp(_) => return,
    };
    for endpoint in endpoints {
        let endpoint_host = endpoint.host_endpoint.as_str().to_string();
        for location in &endpoint.locations {
            out.push(LocationSnapshot {
                listen: listen_label.to_string(),
                endpoint_host: endpoint_host.clone(),
                path: location.path.clone(),
                location_id: location.id,
                id_string: location.id_string.clone(),
                proxy_pass_to_type: location.proxy_pass_to.get_type_as_str().to_string(),
                proxy_pass_to: location.proxy_pass_to.to_string(),
                has_pool: false,
            });
        }
    }
}
