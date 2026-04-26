use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::Ordering, Arc},
};

use my_http_client::MyHttpClientConnector;
use parking_lot::Mutex;

use crate::app::APP_CTX;

use super::{spawn_supervisor, ConnectorFactory, H2Pool, PoolKey, PoolParams};

pub struct H2PoolRegistry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pools: Mutex<HashMap<PoolKey, Arc<H2Pool<TStream, TConnector>>>>,
}

impl<TStream, TConnector> H2PoolRegistry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            pools: Mutex::new(HashMap::new()),
        }
    }

    pub fn ensure_pool(
        &self,
        key: PoolKey,
        params: PoolParams,
        factory: ConnectorFactory<TConnector>,
    ) -> Arc<H2Pool<TStream, TConnector>> {
        let mut pools = self.pools.lock();
        if let Some(existing) = pools.get(&key) {
            return existing.clone();
        }

        let label = key.endpoint_label();
        let pool_size = params.pool_size as i64;
        let pool = Arc::new(H2Pool::new(key.clone(), params, factory));
        pools.insert(key, pool.clone());
        APP_CTX.prometheus.set_h2_pool_size(&label, pool_size);
        spawn_supervisor(pool.clone());
        pool
    }

    pub fn get(&self, key: &PoolKey) -> Option<Arc<H2Pool<TStream, TConnector>>> {
        self.pools.lock().get(key).cloned()
    }

    // Hot-reload support: removes pools whose endpoint is no longer referenced. Not wired
    // yet — needs the configuration reloader to compute the active key-set first.
    #[allow(dead_code)]
    pub fn drain_unused(&self, active_keys: &HashSet<PoolKey>) {
        let mut pools = self.pools.lock();
        pools.retain(|key, pool| {
            if active_keys.contains(key) {
                true
            } else {
                pool.shutdown.store(true, Ordering::Relaxed);
                APP_CTX.prometheus.reset_h2_pool(&key.endpoint_label());
                false
            }
        });
    }

    pub fn snapshot(&self) -> Vec<(PoolKey, usize, usize)> {
        let pools = self.pools.lock();
        pools
            .iter()
            .map(|(k, p)| (k.clone(), p.ready_slots(), p.slots.len()))
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
