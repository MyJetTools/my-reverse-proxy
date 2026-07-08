use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use arc_swap::ArcSwap;
use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector, MyHttpClientError};
use parking_lot::Mutex;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use rust_extensions::sorted_vec::EntityWithKey;

use crate::upstream_status::{AtomicUpstreamStatus, UpstreamStatus};

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
    /// True while a background top-up task is filling the pool to `pool_size`.
    /// Guards against the supervisor stacking multiple top-up tasks.
    pub top_up_pending: AtomicBool,
    pub factory: ConnectorFactory<TConnector>,
    /// Outcome of the most recent connect / revive / health-ping attempt.
    /// Surfaced to the admin UI; not used for routing decisions (the pool
    /// already tracks per-entry `dead` for that).
    pub last_status: AtomicUpstreamStatus,
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
            top_up_pending: AtomicBool::new(false),
            factory,
            last_status: AtomicUpstreamStatus::new(),
        }
    }

    /// Returns a pool entry for the next request. Three internal paths:
    ///
    /// - **Path A (pick-live)** — pool at target: starting from the round-robin
    ///   position, take the first live entry. Dead entries are skipped and
    ///   revived in the background — a request never waits for a reconnect
    ///   while the pool has live capacity.
    /// - **Path B (all dead)** — no live entry: one coalesced foreground
    ///   revive attempt of the round-robin pick. Waiters are bounded by
    ///   `dead_pool_wait_budget` (lock wait) and fail fast inside
    ///   `revive_cooldown`; the single dial-winner ("canary") pays up to
    ///   `connect_timeout` — someone has to dial, and it recovers the pool
    ///   the instant the upstream is back. On any Err the pool is re-scanned:
    ///   a sibling revived meanwhile serves the request instead of a 503.
    /// - **Path 0** — pool below target: connect, then push under `grow_lock`
    ///   with a final size re-check (no overshoot).
    pub async fn get_connection(
        self: &Arc<Self>,
    ) -> Result<Arc<H2Entry<TStream, TConnector>>, MyHttpClientError> {
        let target = self.params.pool_size as usize;
        let snap = self.clients.load();

        if snap.len() < target {
            // Path 0 — grow. Connect first, then push under grow_lock with re-check.
            drop(snap);
            let new_client = self.connect_one().await?;
            let new_entry = Arc::new(H2Entry::new(Arc::new(new_client)));
            // If the push loses the size race the entry is returned as a
            // one-shot (served this request, then dropped) — no overshoot.
            self.try_push(new_entry.clone());
            return Ok(new_entry);
        }

        // Path A — pick-live: first !dead entry starting at the round-robin
        // position. Lock-free; skipped dead entries get a background revive.
        let len = snap.len();
        let start = self.next.fetch_add(1, Ordering::Relaxed) % len;
        for i in 0..len {
            let entry = &snap[(start + i) % len];
            if !entry.dead.load(Ordering::Relaxed) {
                return Ok(entry.clone());
            }
            self.spawn_revive(entry.clone());
        }

        // Path B — every entry is dead. One coalesced foreground attempt on
        // the round-robin pick: wait for its revive_lock only up to
        // dead_pool_wait_budget (rides out an in-flight successful reconnect);
        // the cooldown inside revive_under_lock turns repeat attempts into
        // fast failures instead of serial connect storms.
        let entry = snap[start].clone();
        drop(snap);
        match self.revive_dead_pool(&entry).await {
            Ok(()) => Ok(entry),
            Err(err) => {
                // A sibling may have been revived in the background while we
                // waited — serve from it instead of failing a request against
                // a pool that has live capacity again.
                let snap = self.clients.load();
                let len = snap.len();
                for i in 0..len {
                    let entry = &snap[(start + i) % len];
                    if !entry.dead.load(Ordering::Relaxed) {
                        return Ok(entry.clone());
                    }
                }
                Err(err)
            }
        }
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

    pub(crate) async fn connect_one(
        &self,
    ) -> Result<MyHttp2Client<TStream, TConnector>, MyHttpClientError> {
        let (connector, metrics) = (self.factory)();
        let mut client = MyHttp2Client::new_with_metrics(connector, metrics);
        client.set_connect_timeout(self.params.connect_timeout);
        match client.connect().await {
            Ok(_) => {
                self.last_status.set(UpstreamStatus::Ok);
                Ok(client)
            }
            Err(e) => {
                self.last_status.set(UpstreamStatus::Error);
                Err(e)
            }
        }
    }

    /// Append an already-connected entry to `clients` under `grow_lock`, but
    /// only if the pool is still below `pool_size` (final re-check under the
    /// lock prevents overshoot on a growth race). Returns whether it was
    /// pushed; a `false` means the caller holds a one-shot connection.
    pub(crate) fn try_push(&self, new_entry: Arc<H2Entry<TStream, TConnector>>) -> bool {
        let target = self.params.pool_size as usize;
        let _g = self.grow_lock.lock();
        // A drained pool must not accept a connection whose dial completed
        // after drain_unused decommissioned it.
        if self.shutdown.load(Ordering::Relaxed) {
            return false;
        }
        let cur = self.clients.load_full();
        if cur.len() >= target {
            return false;
        }
        let mut new_vec: Vec<_> = (*cur).clone();
        new_vec.push(new_entry);
        self.clients.store(Arc::new(new_vec));
        true
    }

    /// Revive a dead entry under its `revive_lock` (unbounded lock wait —
    /// background callers only). Foreground all-dead recovery goes through
    /// `revive_dead_pool` instead.
    pub async fn revive_entry(
        &self,
        entry: &Arc<H2Entry<TStream, TConnector>>,
    ) -> Result<(), MyHttpClientError> {
        let _g = entry.revive_lock.lock().await;
        self.revive_under_lock(entry).await
    }

    /// Foreground all-dead recovery: coalesces with any in-flight revive of
    /// this entry. Waits for the `revive_lock` only up to
    /// `dead_pool_wait_budget` — if a parallel attempt succeeds within the
    /// budget, returns Ok having done no work; if one is still (or repeatedly)
    /// failing, fails fast instead of queueing behind it.
    async fn revive_dead_pool(
        &self,
        entry: &Arc<H2Entry<TStream, TConnector>>,
    ) -> Result<(), MyHttpClientError> {
        let lock_result =
            tokio::time::timeout(self.params.dead_pool_wait_budget, entry.revive_lock.lock())
                .await;
        let Ok(_g) = lock_result else {
            return Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                "'{}': all pool connections are dead, a reconnect is already in progress",
                self.desc.name
            )));
        };
        self.revive_under_lock(entry).await
    }

    /// Caller must hold `entry.revive_lock`. Re-checks `dead` (a parallel
    /// caller may have already revived), then rate-limits actual connect
    /// attempts by `revive_cooldown` so a down upstream costs at most one
    /// dial per window per entry — everyone else fails fast.
    async fn revive_under_lock(
        &self,
        entry: &Arc<H2Entry<TStream, TConnector>>,
    ) -> Result<(), MyHttpClientError> {
        if !entry.dead.load(Ordering::Relaxed) {
            return Ok(());
        }

        if let Some(remaining) = entry.revive_cooldown_remaining(self.params.revive_cooldown) {
            return Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                "'{}': upstream is down; reconnect is rate-limited for another {:?}",
                self.desc.name, remaining
            )));
        }
        entry
            .last_revive_attempt
            .update(DateTimeAsMicroseconds::now());

        let new_client = self.connect_one().await?;
        entry.client.store(Arc::new(new_client));
        entry
            .last_success
            .update(DateTimeAsMicroseconds::now());
        entry.dead.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn last_status(&self) -> UpstreamStatus {
        self.last_status.get()
    }

    /// Publishes the alive gauge unless the pool is drained; re-checks
    /// `shutdown` AFTER the write and undoes it — `drain_unused` may reset
    /// the gauge concurrently, and a removed pool's label must stay reset
    /// (nothing will ever reset it again).
    pub(crate) fn publish_alive_gauge(&self) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        crate::app::APP_CTX
            .prometheus
            .set_h2_pool_alive(&self.desc.name, self.alive_count() as i64);
        if self.shutdown.load(Ordering::Relaxed) {
            crate::app::APP_CTX.prometheus.reset_h2_pool(&self.desc.name);
        }
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
