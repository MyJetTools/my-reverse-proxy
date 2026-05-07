use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use arc_swap::ArcSwap;
use my_http_client::{http1::MyHttpClient, MyHttpClientConnector, MyHttpClientError};
use parking_lot::Mutex;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use rust_extensions::sorted_vec::EntityWithKey;

use super::{ConnectorFactory, H1ClientHandle, H1Entry, PoolDesc, PoolParams, DISPOSABLE_COUNTER, MAX_DISPOSABLE};

const OVERFLOW_RETRY_SLEEP: Duration = Duration::from_millis(10);

pub struct H1Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub desc: PoolDesc,
    pub params: PoolParams,
    pub clients: ArcSwap<Vec<Arc<H1Entry<TStream, TConnector>>>>,
    /// Held briefly (no await) only during a Path 0 push. Connect happens
    /// before acquiring this — never held across await.
    pub grow_lock: Mutex<()>,
    pub next: AtomicUsize,
    pub shutdown: AtomicBool,
    pub factory: ConnectorFactory<TConnector>,
}

impl<TStream, TConnector> H1Pool<TStream, TConnector>
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

    /// Acquires a client handle for one in-flight h1 request. Three-phase loop:
    ///
    /// - **Phase 0** — pool below target: connect a fresh client, push it
    ///   pre-rented under `grow_lock` with re-check; if race lost, hand it out
    ///   as Disposable (counter inc'd).
    /// - **Phase 1** — pool at target: round-robin scan, `compare_exchange` to
    ///   rent the first free entry. Alive → Path A. Dead → Path B (revive).
    /// - **Phase 2** — pool full and all rented: overflow disposable up to
    ///   `MAX_DISPOSABLE`. Above limit → sleep 10ms and retry whole loop.
    pub async fn get_connection(
        &self,
    ) -> Result<H1ClientHandle<TStream, TConnector>, MyHttpClientError> {
        let target = self.params.pool_size as usize;

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                return Err(MyHttpClientError::Disposed);
            }

            let snap = self.clients.load_full();
            let len = snap.len();

            // Phase 0 — grow
            if len < target {
                let new_client = Arc::new(self.connect_one().await?);
                let new_entry = Arc::new(H1Entry::new(new_client.clone()));
                new_entry.rented.store(true, Ordering::Relaxed);

                let _g = self.grow_lock.lock();
                let cur = self.clients.load_full();
                if cur.len() < target {
                    let mut new_vec: Vec<_> = (*cur).clone();
                    new_vec.push(new_entry.clone());
                    self.clients.store(Arc::new(new_vec));
                    drop(_g);
                    return Ok(H1ClientHandle::reusable(new_client, new_entry));
                }
                // Race lost — pool already at target. Don't waste the connect:
                // hand it out as Disposable (counter inc'd so Drop dec'es).
                drop(_g);
                DISPOSABLE_COUNTER.fetch_add(1, Ordering::Relaxed);
                return Ok(H1ClientHandle::disposable(new_client));
            }

            // Phase 1 — rent scan (pool at target).
            let start = self.next.fetch_add(1, Ordering::Relaxed) % len;
            for offset in 0..len {
                let i = (start + offset) % len;
                let entry = &snap[i];
                if !entry.try_rent() {
                    continue;
                }
                if !entry.dead.load(Ordering::Relaxed) {
                    // Path A
                    let client = entry.client.load_full();
                    return Ok(H1ClientHandle::reusable(client, entry.clone()));
                }
                // Path B — revive under per-entry lock.
                match self.revive_entry(entry).await {
                    Ok(()) => {
                        let client = entry.client.load_full();
                        return Ok(H1ClientHandle::reusable(client, entry.clone()));
                    }
                    Err(e) => {
                        entry.release_rent();
                        return Err(e);
                    }
                }
            }

            // Phase 2 — all rented; need overflow disposable.
            {
                let cur = DISPOSABLE_COUNTER.fetch_add(1, Ordering::Relaxed);
                if cur < MAX_DISPOSABLE {
                    match self.connect_one().await {
                        Ok(client) => {
                            return Ok(H1ClientHandle::disposable(Arc::new(client)));
                        }
                        Err(e) => {
                            // Connect failed — undo counter inc and bubble up.
                            DISPOSABLE_COUNTER.fetch_sub(1, Ordering::Relaxed);
                            return Err(e);
                        }
                    }
                }
                // Limit reached — undo and sleep before retrying.
                DISPOSABLE_COUNTER.fetch_sub(1, Ordering::Relaxed);
                tokio::time::sleep(OVERFLOW_RETRY_SLEEP).await;
            }
        }
    }

    /// Fresh `MyHttpClient` for a WebSocket session. Not stored anywhere; not
    /// counted against `DISPOSABLE_COUNTER`. The handle's Drop is a no-op —
    /// the underlying TCP closes via `MyHttpClient::Drop` when the WS Arc dies.
    pub async fn create_ws_connection(
        &self,
    ) -> Result<H1ClientHandle<TStream, TConnector>, MyHttpClientError> {
        let client = Arc::new(self.connect_one().await?);
        Ok(H1ClientHandle::ws(client))
    }

    async fn connect_one(&self) -> Result<MyHttpClient<TStream, TConnector>, MyHttpClientError> {
        let (connector, metrics) = (self.factory)();
        let mut client = MyHttpClient::new_with_metrics(connector, metrics);
        client.set_connect_timeout(self.params.connect_timeout);
        client.connect().await?;
        Ok(client)
    }

    /// Revive a dead entry under its `revive_lock`. Used by Path B (foreground)
    /// and the supervisor's spawned revive task. Re-checks `dead` after lock —
    /// if already revived by a parallel caller, returns Ok without doing work.
    pub async fn revive_entry(
        &self,
        entry: &Arc<H1Entry<TStream, TConnector>>,
    ) -> Result<(), MyHttpClientError> {
        let _g = entry.revive_lock.lock().await;
        if !entry.dead.load(Ordering::Relaxed) {
            return Ok(());
        }
        let new_client = self.connect_one().await?;
        entry.client.store(Arc::new(new_client));
        entry.last_success.update(DateTimeAsMicroseconds::now());
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

impl<TStream, TConnector> EntityWithKey<i64> for H1Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn get_key(&self) -> &i64 {
        &self.desc.location_id
    }
}
