use std::sync::{atomic::Ordering, Arc};

use ahash::{AHashMap, AHashSet};
use arc_swap::ArcSwap;
use my_http_client::MyHttpClientConnector;
use parking_lot::Mutex;

use crate::app::APP_CTX;

use super::{ConnectorFactory, H2Pool, PoolKey, PoolParams};

pub struct H2PoolRegistry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    /// Lock-free reads via `load()` for the hot get/list/snapshot paths.
    pools: ArcSwap<AHashMap<PoolKey, Arc<H2Pool<TStream, TConnector>>>>,
    /// Serializes writes (`ensure_pool`, `drain_unused`) so they don't race
    /// with each other and accidentally drop a freshly-added pool. Held
    /// briefly — no `await` inside.
    write_lock: Mutex<()>,
}

impl<TStream, TConnector> H2PoolRegistry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            pools: ArcSwap::from_pointee(AHashMap::new()),
            write_lock: Mutex::new(()),
        }
    }

    pub fn ensure_pool(
        &self,
        key: PoolKey,
        params: PoolParams,
        factory: ConnectorFactory<TConnector>,
    ) -> Arc<H2Pool<TStream, TConnector>> {
        if let Some(existing) = self.pools.load().get(&key) {
            return existing.clone();
        }

        let _g = self.write_lock.lock();
        let cur = self.pools.load_full();
        if let Some(existing) = cur.get(&key) {
            return existing.clone();
        }

        let label = key.endpoint_label();
        let pool_size = params.pool_size as i64;
        let pool = Arc::new(H2Pool::new(key.clone(), params, factory));
        let mut new_map = (*cur).clone();
        new_map.insert(key, pool.clone());
        self.pools.store(Arc::new(new_map));
        APP_CTX.prometheus.set_h2_pool_size(&label, pool_size);
        pool
    }

    pub fn get(&self, key: &PoolKey) -> Option<Arc<H2Pool<TStream, TConnector>>> {
        self.pools.load().get(key).cloned()
    }

    pub fn list_pools(&self) -> Vec<Arc<H2Pool<TStream, TConnector>>> {
        self.pools.load().values().cloned().collect()
    }

    /// Removes pools whose endpoint is no longer referenced. Called periodically
    /// by `GcPoolsTimer`.
    pub fn drain_unused(&self, active_keys: &AHashSet<PoolKey>) {
        let _g = self.write_lock.lock();
        let cur = self.pools.load_full();
        let mut new_map = AHashMap::new();
        for (key, pool) in cur.iter() {
            if active_keys.contains(key) {
                new_map.insert(key.clone(), pool.clone());
            } else {
                pool.shutdown.store(true, Ordering::Relaxed);
                APP_CTX.prometheus.reset_h2_pool(&key.endpoint_label());
            }
        }
        self.pools.store(Arc::new(new_map));
    }

    pub fn snapshot(&self) -> Vec<(PoolKey, usize, usize)> {
        self.pools
            .load()
            .iter()
            .map(|(k, p)| (k.clone(), p.alive_count(), p.total_count()))
            .collect()
    }
}

impl<TStream, TConnector> Default for H2PoolRegistry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
