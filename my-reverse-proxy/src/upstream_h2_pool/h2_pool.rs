use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use arc_swap::ArcSwap;
use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector, MyHttpClientError};
use parking_lot::Mutex;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use rust_extensions::sorted_vec::EntityWithKey;

use super::{ConnectorFactory, H2Entry, PoolDesc, PoolParams};

pub struct H2Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub desc: PoolDesc,
    pub params: PoolParams,
    pub clients: ArcSwap<Vec<Arc<H2Entry<TStream, TConnector>>>>,
    /// Held briefly (no await) while pushing a new entry into the vec.
    /// Connect happens BEFORE acquiring this — never held across await.
    pub grow_lock: Mutex<()>,
    pub next: AtomicUsize,
    pub shutdown: AtomicBool,
    pub factory: ConnectorFactory<TConnector>,
}

impl<TStream, TConnector> H2Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new(
        desc: PoolDesc,
        params: PoolParams,
        factory: ConnectorFactory<TConnector>,
    ) -> Self {
        Self {
            desc,
            params,
            clients: ArcSwap::from_pointee(Vec::new()),
            grow_lock: Mutex::new(()),
            next: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            factory,
        }
    }

    /// Returns a pool entry for the next request. Three internal paths:
    ///
    /// - **Path A** — pool at target & round-robin pick is live: lock-free clone.
    /// - **Path B** — pool at target & round-robin pick is dead: revive under
    ///   `entry.revive_lock` (serializes vs background revive_task).
    /// - **Path 0** — pool below target: connect, then push under `grow_lock`
    ///   with a final size re-check (no overshoot).
    pub async fn get_connection(
        &self,
    ) -> Result<Arc<H2Entry<TStream, TConnector>>, MyHttpClientError> {
        let target = self.params.pool_size as usize;
        let snap = self.clients.load();

        if snap.len() < target {
            // Path 0 — grow. Connect first, then push under grow_lock with re-check.
            drop(snap);
            let new_client = self.connect_one().await?;
            let new_entry = Arc::new(H2Entry::new(Arc::new(new_client)));

            let _g = self.grow_lock.lock();
            let cur = self.clients.load_full();
            if cur.len() < target {
                let mut new_vec: Vec<_> = (*cur).clone();
                new_vec.push(new_entry.clone());
                self.clients.store(Arc::new(new_vec));
            }
            // else: race lost — pool already at target. new_entry returned as one-shot.
            return Ok(new_entry);
        }

        // Path A/B — pool at target, pick by round-robin.
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % snap.len();
        let entry = snap[idx].clone();
        drop(snap);

        // Lock-free dead check — hot path.
        if !entry.dead.load(Ordering::Relaxed) {
            // Path A
            return Ok(entry);
        }

        // Path B — revive under per-entry lock.
        self.revive_entry(&entry).await?;
        Ok(entry)
    }

    /// Fresh client created via the same factory the pool uses, but never
    /// stored — caller owns its lifetime via the returned Arc. Used by the
    /// WS extended-CONNECT fast path.
    pub async fn create_connection(
        &self,
    ) -> Result<Arc<MyHttp2Client<TStream, TConnector>>, MyHttpClientError> {
        let client = self.connect_one().await?;
        Ok(Arc::new(client))
    }

    async fn connect_one(&self) -> Result<MyHttp2Client<TStream, TConnector>, MyHttpClientError> {
        let (connector, metrics) = (self.factory)();
        let mut client = MyHttp2Client::new_with_metrics(connector, metrics);
        client.set_connect_timeout(self.params.connect_timeout);
        client.connect().await?;
        Ok(client)
    }

    /// Revive a dead entry under its `revive_lock`. Called by Path B (foreground)
    /// and by the supervisor's spawned revive task. After acquiring the lock
    /// re-checks `dead` — if already revived by a parallel caller, returns Ok
    /// without doing any work.
    pub async fn revive_entry(
        &self,
        entry: &Arc<H2Entry<TStream, TConnector>>,
    ) -> Result<(), MyHttpClientError> {
        let _g = entry.revive_lock.lock().await;
        if !entry.dead.load(Ordering::Relaxed) {
            return Ok(());
        }
        let new_client = self.connect_one().await?;
        entry.client.store(Arc::new(new_client));
        entry
            .last_success
            .update(DateTimeAsMicroseconds::now());
        entry.dead.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn alive_count(&self) -> usize {
        self.clients
            .load()
            .iter()
            .filter(|e| !e.dead.load(Ordering::Relaxed))
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.clients.load().len()
    }
}

impl<TStream, TConnector> EntityWithKey<i64> for H2Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn get_key(&self) -> &i64 {
        &self.desc.location_id
    }
}
