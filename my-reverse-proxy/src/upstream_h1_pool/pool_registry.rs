use std::sync::{atomic::Ordering, Arc};

use ahash::AHashSet;
use arc_swap::ArcSwap;
use my_http_client::MyHttpClientConnector;
use parking_lot::Mutex;
use rust_extensions::sorted_vec::SortedVecOfArc;

use crate::app::APP_CTX;

use super::{ConnectorFactory, H1Pool, PoolDesc, PoolParams};

pub struct H1PoolRegistry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    /// Lock-free reads via `load()` for the hot get/list/snapshot paths.
    /// Pools are keyed by `location_id` — ids come from a process-global
    /// monotonic counter, so each location maps to its own pool.
    pools: ArcSwap<SortedVecOfArc<i64, H1Pool<TStream, TConnector>>>,
    /// Serializes writes (`ensure_pool`, `drain_unused`) so they don't race
    /// with each other and accidentally drop a freshly-added pool. Held
    /// briefly — no `await` inside.
    write_lock: Mutex<()>,
}

impl<TStream, TConnector> H1PoolRegistry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            pools: ArcSwap::from_pointee(SortedVecOfArc::new()),
            write_lock: Mutex::new(()),
        }
    }

    pub fn ensure_pool(
        &self,
        desc: PoolDesc,
        params: PoolParams,
        factory: ConnectorFactory<TConnector>,
    ) -> Arc<H1Pool<TStream, TConnector>> {
        if let Some(existing) = self.pools.load().get(&desc.location_id) {
            return existing.clone();
        }

        let _g = self.write_lock.lock();
        let cur = self.pools.load_full();
        if let Some(existing) = cur.get(&desc.location_id) {
            return existing.clone();
        }

        let name = desc.name.clone();
        let pool_size = params.pool_size as i64;
        let pool = Arc::new(H1Pool::new(desc, params, factory));
        let mut new_vec: SortedVecOfArc<i64, H1Pool<TStream, TConnector>> =
            SortedVecOfArc::from_iterator(cur.iter().cloned());
        new_vec.insert_or_replace(pool.clone());
        self.pools.store(Arc::new(new_vec));
        APP_CTX.prometheus.set_h1_pool_size(&name, pool_size);
        pool
    }

    pub fn list_pools(&self) -> Vec<Arc<H1Pool<TStream, TConnector>>> {
        self.pools.load().iter().cloned().collect()
    }

    pub fn get(&self, location_id: i64) -> Option<Arc<H1Pool<TStream, TConnector>>> {
        self.pools.load().get(&location_id).cloned()
    }

    /// Removes pools whose location is no longer referenced. Called periodically
    /// by `GcPoolsTimer`.
    pub fn drain_unused(&self, active: &AHashSet<i64>) {
        let _g = self.write_lock.lock();
        let cur = self.pools.load_full();
        let mut new_vec: SortedVecOfArc<i64, H1Pool<TStream, TConnector>> = SortedVecOfArc::new();
        for pool in cur.iter() {
            if active.contains(&pool.desc.location_id) {
                new_vec.insert_or_replace(pool.clone());
            } else {
                pool.shutdown.store(true, Ordering::Relaxed);
                APP_CTX.prometheus.reset_h1_pool(&pool.desc.name);
            }
        }
        self.pools.store(Arc::new(new_vec));
    }

    pub fn snapshot(&self) -> Vec<(String, usize, usize)> {
        self.pools
            .load()
            .iter()
            .map(|p| (p.desc.name.clone(), p.alive_count(), p.total_count()))
            .collect()
    }
}

impl<TStream, TConnector> Default for H1PoolRegistry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
