use ahash::AHashSet;
use rust_extensions::{MyTimerTick, RepeatTimerIteration};

use crate::{
    app::APP_CTX,
    configurations::{
        ListenConfiguration, MyReverseProxyRemoteEndpoint, ProxyPassLocationConfig,
        ProxyPassToConfig,
    },
};

/// Periodic GC for the per-location upstream pools. Removes pools whose
/// location is no longer referenced by any location in the current
/// configuration. Pools are created lazily on first request — this timer is
/// the only mechanism that removes them.
pub struct GcPoolsTimer;

#[async_trait::async_trait]
impl MyTimerTick for GcPoolsTimer {
    async fn tick(&self) -> RepeatTimerIteration {
        let desired = APP_CTX
            .current_configuration
            .get(|cfg| collect_desired_keys(cfg))
            .await;

        APP_CTX.h1_tcp_pools.drain_unused(&desired.h1_tcp);
        APP_CTX.h1_tls_pools.drain_unused(&desired.h1_tls);
        APP_CTX.h1_uds_pools.drain_unused(&desired.h1_uds);
        APP_CTX.h2_tcp_pools.drain_unused(&desired.h2_tcp);
        APP_CTX.h2_tls_pools.drain_unused(&desired.h2_tls);
        APP_CTX.h2_uds_pools.drain_unused(&desired.h2_uds);

        RepeatTimerIteration::WithInterval
    }
}

#[derive(Default)]
struct DesiredKeys {
    h1_tcp: AHashSet<i64>,
    h1_tls: AHashSet<i64>,
    h1_uds: AHashSet<i64>,
    h2_tcp: AHashSet<i64>,
    h2_tls: AHashSet<i64>,
    h2_uds: AHashSet<i64>,
}

fn collect_desired_keys(cfg: &crate::configurations::AppConfigurationInner) -> DesiredKeys {
    let mut out = DesiredKeys::default();

    let walk_listen = |listen: &ListenConfiguration, out: &mut DesiredKeys| match listen {
        ListenConfiguration::Http(http) | ListenConfiguration::Mcp(http) => {
            for endpoint in &http.endpoints {
                for location in &endpoint.locations {
                    absorb_location(location.as_ref(), out);
                }
            }
        }
        ListenConfiguration::Tcp(_) => {}
    };

    for listen in cfg.listen_tcp_endpoints.values() {
        walk_listen(listen, &mut out);
    }
    for listen in cfg.listen_unix_socket_endpoints.values() {
        walk_listen(listen, &mut out);
    }

    out
}

/// Which pool family a location's upstream lives in. `mcp` / `mcp-h2` are
/// `http1` / `http2` upstreams — they pick their connection from the very same
/// pools, so they must be counted as desired here too, or their pool would be
/// drained on every tick.
enum PoolFamily {
    H1,
    H2,
}

fn absorb_location(location: &ProxyPassLocationConfig, out: &mut DesiredKeys) {
    let location_id = location.id;

    // Unix socket variants short-circuit: their `remote_host` is a filesystem
    // path with no URL scheme, so `get_scheme()` would return None below and
    // we'd skip the location — leaving the pool orphaned and drained on the
    // next tick.
    let (family, model) = match &location.proxy_pass_to {
        ProxyPassToConfig::UnixHttp1(_) => {
            out.h1_uds.insert(location_id);
            return;
        }
        ProxyPassToConfig::UnixHttp2(_) => {
            out.h2_uds.insert(location_id);
            return;
        }
        ProxyPassToConfig::Http1(m) | ProxyPassToConfig::McpHttp1(m) => (PoolFamily::H1, m),
        ProxyPassToConfig::Http2(m) | ProxyPassToConfig::McpHttp2(m) => (PoolFamily::H2, m),
        _ => return,
    };

    let MyReverseProxyRemoteEndpoint::Direct { remote_host } = &model.remote_host else {
        // Gateway / OverSsh routes don't use h1/h2 pools.
        return;
    };

    let Some(scheme) = remote_host.get_scheme() else {
        return;
    };

    // Mirrors the factory `create_data_source` picks for the same pair — an h2
    // location with a ws/wss upstream is served by the h1 pools.
    use rust_extensions::remote_endpoint::Scheme;
    match family {
        PoolFamily::H1 => match scheme {
            Scheme::Http | Scheme::Ws => {
                out.h1_tcp.insert(location_id);
            }
            Scheme::Https | Scheme::Wss => {
                out.h1_tls.insert(location_id);
            }
            Scheme::UnixSocket => {
                out.h1_uds.insert(location_id);
            }
        },
        PoolFamily::H2 => match scheme {
            Scheme::Http => {
                out.h2_tcp.insert(location_id);
            }
            Scheme::Https => {
                out.h2_tls.insert(location_id);
            }
            Scheme::Ws => {
                out.h1_tcp.insert(location_id);
            }
            Scheme::Wss => {
                out.h1_tls.insert(location_id);
            }
            Scheme::UnixSocket => {
                out.h2_uds.insert(location_id);
            }
        },
    }
}
