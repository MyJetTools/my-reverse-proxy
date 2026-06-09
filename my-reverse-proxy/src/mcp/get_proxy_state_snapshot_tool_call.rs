use std::sync::atomic::Ordering;

use ahash::AHashSet;
use mcp_server_middleware::*;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::*;

use crate::app::APP_CTX;
use crate::configurations::{AppConfigurationInner, ListenConfiguration};
use crate::upstream_h1_pool::{DISPOSABLE_COUNTER, MAX_DISPOSABLE};
use crate::upstream_status::UpstreamStatus;

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
        description = "What the proxy itself believes about this upstream: outcome of the most recent connect / revive / health-ping. 'ok' = last attempt succeeded; 'error' = last attempt failed (the proxy KNOWS it is down); 'unknown' = no attempt made yet. This is the answer to 'do we see that it fell down'."
    )]
    pub last_status: String,

    #[property(
        description = "h1 only: number of on-demand (disposable) connections currently in flight for THIS upstream — created when every pooled entry is rented (Phase 0 race-loser or Phase 2 overflow), counted down when the request finishes. null for h2 pools, which are multiplexed and never open on-demand connections. See live_disposables_global / max_disposables_global on the top-level response for the process-wide budget."
    )]
    pub live_disposables: Option<i64>,

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

    #[property(
        description = "Process-wide count of live on-demand (disposable) h1 connections across ALL pools (sum of every pool's live_disposables). This is the value checked against max_disposables_global before a new overflow connection is opened."
    )]
    pub live_disposables_global: i64,

    #[property(
        description = "Hard cap on concurrent on-demand (disposable) h1 connections process-wide (MAX_DISPOSABLE). When live_disposables_global reaches this, get_connection stops opening overflow connections and retries every 10ms instead — a sustained value at the cap means upstreams are saturated."
    )]
    pub max_disposables_global: i64,
}

pub struct GetProxyStateSnapshotHandler;

impl ToolDefinition for GetProxyStateSnapshotHandler {
    const FUNC_NAME: &'static str = "get_proxy_state_snapshot";
    const DESCRIPTION: &'static str = "Detailed snapshot of all upstream pools across the 6 pool registries (h1/h2 × tcp/tls/uds) and all locations from the current configuration. Per pool: alive/total entry counts, last_status (what the proxy believes about the upstream — 'ok'/'error'/'unknown' from the last connect/revive/health-ping, i.e. whether WE see it is down), live_disposables (on-demand connections in flight for this upstream — h1 only), shutdown flag, id_string, and per-entry state (dead / last_success / idle_secs / rented). Top-level: live_disposables_global / max_disposables_global (process-wide on-demand budget) and orphan/naked correlation. Use to see, per upstream, which connections are pooled vs on-demand and whether the proxy has detected it as down.";
}

#[async_trait::async_trait]
impl McpToolCall<GetProxyStateSnapshotInputData, GetProxyStateSnapshotResponse>
    for GetProxyStateSnapshotHandler
{
    async fn execute_tool_call(
        &self,
        _model: GetProxyStateSnapshotInputData,
    ) -> Result<GetProxyStateSnapshotResponse, String> {
        Ok(build_proxy_state_snapshot().await)
    }
}

/// Builds the full proxy-state snapshot (all pools + locations + correlation +
/// on-demand budget). Shared by the `get_proxy_state_snapshot` MCP tool and the
/// `/api/debug/upstreams-snapshot` REST endpoint so both return identical data.
pub async fn build_proxy_state_snapshot() -> GetProxyStateSnapshotResponse {
    {
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

        GetProxyStateSnapshotResponse {
            captured_at: now.to_rfc3339(),
            pools,
            locations,
            orphan_pool_location_ids,
            naked_location_ids,
            live_disposables_global: DISPOSABLE_COUNTER.load(Ordering::Relaxed) as i64,
            max_disposables_global: MAX_DISPOSABLE as i64,
        }
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
        .map(|pool| build_h1_pool_snapshot(pool, registry, now))
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
        .map(|pool| build_h2_pool_snapshot(pool, registry, now))
        .collect()
}

fn upstream_status_str(status: UpstreamStatus) -> String {
    match status {
        UpstreamStatus::Ok => "ok",
        UpstreamStatus::Error => "error",
        UpstreamStatus::Unknown => "unknown",
    }
    .to_string()
}

fn entry_idle_secs(last: DateTimeAsMicroseconds, now: DateTimeAsMicroseconds) -> i64 {
    now.duration_since(last).as_positive_or_zero().as_secs() as i64
}

/// Builds the snapshot for a single h1 pool. Shared by `get_proxy_state_snapshot`
/// and `lookup_pool` so the per-entry / per-pool shape stays identical between
/// the two tools. `has_matching_location` is left false — the snapshot tool fills
/// it in a second pass; lookup leaves it false (it has no config to correlate).
pub(super) fn build_h1_pool_snapshot<TStream, TConnector>(
    pool: &std::sync::Arc<crate::upstream_h1_pool::H1Pool<TStream, TConnector>>,
    registry: &str,
    now: DateTimeAsMicroseconds,
) -> PoolSnapshot
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: my_http_client::MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let entries: Vec<EntrySnapshot> = pool
        .clients
        .load()
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let last = entry.last_success.as_date_time();
            EntrySnapshot {
                index: i as i64,
                dead: entry.dead.load(Ordering::Relaxed),
                last_success: last.to_rfc3339(),
                idle_secs: entry_idle_secs(last, now),
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
        last_status: upstream_status_str(pool.last_status()),
        live_disposables: Some(pool.live_disposables() as i64),
        shutdown: pool.shutdown.load(Ordering::Relaxed),
        entries,
        has_matching_location: false,
    }
}

/// Builds the snapshot for a single h2 pool. h2 entries have no `rented` flag and
/// h2 never opens on-demand connections, so `rented` / `live_disposables` are null.
pub(super) fn build_h2_pool_snapshot<TStream, TConnector>(
    pool: &std::sync::Arc<crate::upstream_h2_pool::H2Pool<TStream, TConnector>>,
    registry: &str,
    now: DateTimeAsMicroseconds,
) -> PoolSnapshot
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: my_http_client::MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let entries: Vec<EntrySnapshot> = pool
        .clients
        .load()
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let last = entry.last_success.as_date_time();
            EntrySnapshot {
                index: i as i64,
                dead: entry.dead.load(Ordering::Relaxed),
                last_success: last.to_rfc3339(),
                idle_secs: entry_idle_secs(last, now),
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
        last_status: upstream_status_str(pool.last_status()),
        live_disposables: None,
        shutdown: pool.shutdown.load(Ordering::Relaxed),
        entries,
        has_matching_location: false,
    }
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
