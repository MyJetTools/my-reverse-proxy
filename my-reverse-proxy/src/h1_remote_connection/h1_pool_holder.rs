use std::sync::Arc;

use ahash::AHashMap;
use parking_lot::Mutex;

use crate::configurations::ProxyPassToConfig;
use crate::network_stream::NetworkError;

use super::{connection_key, OwnedUpstream, Upstream};

/// Holds H1 upstream connections keyed by upstream identity. ONE interface, two
/// placements:
/// - global: a single process-wide `Arc<H1PoolHolder>` shared by every client
///   connection (cross-connection reuse, `max_idle_per_key > 1`);
/// - local: a fresh `Arc<H1PoolHolder>` per client connection (mcp, or when
///   global reuse is not wanted — `max_idle_per_key == 1`).
///
/// The only difference is ownership + capacity; the API is identical.
///
/// Connections are checked OUT by value ([`acquire`](Self::acquire)) — the
/// worker owns both halves for the request — and returned for reuse
/// ([`release`](Self::release)) on clean keep-alive completion. A connection
/// that is NOT returned is, by definition, dead: the next `acquire` for that
/// key finds no live idle entry and reconnects. So `acquire` is the single
/// recreation point and the single source of "upstream is not available"
/// (`Err`).
pub struct H1PoolHolder {
    pools: Mutex<AHashMap<String, Vec<OwnedUpstream>>>,
    max_idle_per_key: usize,
}

impl H1PoolHolder {
    /// Per-connection pool: at most one idle keep-alive connection per upstream.
    pub fn new_local() -> Arc<Self> {
        Arc::new(Self {
            pools: Mutex::new(AHashMap::new()),
            max_idle_per_key: 1,
        })
    }

    /// Shared pool keeping up to `max_idle_per_key` idle connections per upstream.
    pub fn new_global(max_idle_per_key: usize) -> Arc<Self> {
        Arc::new(Self {
            pools: Mutex::new(AHashMap::new()),
            max_idle_per_key: max_idle_per_key.max(1),
        })
    }

    /// Hand out an exclusively-owned working connection for `proxy_pass_to`:
    /// reuse a live idle one if present (dropping any that died while idle),
    /// otherwise connect a fresh one. `Err` means the upstream could not be
    /// reached at all — the caller renders "upstream is not available".
    ///
    /// The returned `bool` is `true` when the connection was REUSED from the
    /// idle set, `false` when freshly connected — this drives [`ReconnectPolicy`]:
    /// a reused connection that then fails the head send is merely stale (retry),
    /// a fresh one that fails is a broken upstream (give up).
    pub async fn acquire(
        &self,
        proxy_pass_to: &ProxyPassToConfig,
    ) -> Result<(OwnedUpstream, bool), NetworkError> {
        let key = connection_key(proxy_pass_to);

        // Drain dead idle connections; hand out the first live one.
        loop {
            let candidate = {
                let mut pools = self.pools.lock();
                pools.get_mut(&key).and_then(|idle| idle.pop())
            };
            match candidate {
                // A connection can die while idle (nobody is reading it, so the
                // disconnect flag may only flip on the next probe) — skip it.
                Some(owned) if owned.upstream.is_disconnected() => continue,
                Some(owned) => return Ok((owned, true)),
                None => break,
            }
        }

        let owned = Upstream::connect_owned(proxy_pass_to).await?;
        Ok((owned, false))
    }

    /// Return a still-usable connection to the idle set for reuse. A
    /// disconnected connection, or one over the per-key capacity, is dropped
    /// (closed) instead.
    pub fn release(&self, proxy_pass_to: &ProxyPassToConfig, owned: OwnedUpstream) {
        if owned.upstream.is_disconnected() {
            return;
        }
        let key = connection_key(proxy_pass_to);
        let mut pools = self.pools.lock();
        let idle = pools.entry(key).or_default();
        if idle.len() < self.max_idle_per_key {
            idle.push(owned);
        }
    }
}
