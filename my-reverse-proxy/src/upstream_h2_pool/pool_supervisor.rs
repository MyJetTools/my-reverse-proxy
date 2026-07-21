use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use my_http_client::{
    http2::MyHttp2Client, hyper::MyHttpHyperClientMetrics, MyHttpClientConnector,
};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::upstream_status::UpstreamStatus;

use super::{H2Entry, H2Pool};

pub type ConnectorFactory<TConnector> = Arc<
    dyn Fn() -> (
            TConnector,
            Arc<dyn MyHttpHyperClientMetrics + Send + Sync + 'static>,
        ) + Send
        + Sync
        + 'static,
>;

impl<TStream, TConnector> H2Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    /// One supervisor pass:
    /// - pool below `pool_size` (but not empty) → spawn a background top-up
    ///   task so the pool warms to full without waiting for N concurrent
    ///   requests.
    /// - dead → spawn a background revive task (uses entry.revive_lock).
    /// - !dead AND `now - last_success < hot_window` → skip (hot, no probe).
    /// - !dead AND idle AND `health_check_path` set → ping. Fail → mark dead +
    ///   spawn revive.
    pub async fn supervisor_tick(self: &Arc<Self>) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }

        self.spawn_top_up();

        let snap = self.clients.load_full();
        let now = DateTimeAsMicroseconds::now();

        for entry in snap.iter() {
            if self.shutdown.load(Ordering::Relaxed) {
                return;
            }

            if entry.dead.load(Ordering::Relaxed) {
                self.spawn_revive(entry.clone());
                continue;
            }

            // Transport already knows it's gone (keep-alive PING missed,
            // GOAWAY, broken pipe) — no need to wait for the HTTP probe, and
            // this works even when no health_check_path is configured.
            if !entry.client.load().is_alive() {
                entry.dead.store(true, Ordering::Relaxed);
                self.spawn_revive(entry.clone());
                continue;
            }

            let idle = now
                .duration_since(entry.last_success.as_date_time())
                .as_positive_or_zero();
            if idle < self.params.hot_window {
                continue;
            }

            let Some(path) = self.params.health_check_path.as_deref() else {
                continue;
            };
            // An invalid configured path fails the request builder, which
            // would read as "ping failed" and dead-mark a healthy connection
            // every tick. A config typo must not churn connections.
            if !liveness_path_is_valid(path) {
                continue;
            }

            let alive =
                ping_entry(entry, path, &self.desc.authority, self.params.ping_timeout).await;
            if alive {
                entry.last_success.update(DateTimeAsMicroseconds::now());
                self.last_status.set(UpstreamStatus::Ok);
            } else {
                entry.dead.store(true, Ordering::Relaxed);
                self.last_status.set(UpstreamStatus::Error);
                self.spawn_revive(entry.clone());
            }
        }

        self.publish_alive_gauge();
    }

    /// Kick off a background revive for a dead entry. Called from both the
    /// supervisor tick and the pick-live scan in `get_connection`, so it
    /// deduplicates aggressively: skips if the entry is no longer dead, if
    /// the last attempt is still inside the cooldown window, or if another
    /// revive task is already in flight (`revive_pending`).
    pub fn spawn_revive(self: &Arc<Self>, entry: Arc<H2Entry<TStream, TConnector>>) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        if !entry.dead.load(Ordering::Relaxed) {
            return;
        }
        // One-shot entries (Path 0 losers) never made it into `clients` —
        // reviving an orphan would dial a connection nobody can ever pick.
        if !self.clients.load().iter().any(|e| Arc::ptr_eq(e, &entry)) {
            return;
        }
        if entry
            .revive_cooldown_remaining(self.params.revive_cooldown)
            .is_some()
        {
            return;
        }
        if entry
            .revive_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let pool = self.clone();
        // The guard owns the entry and clears revive_pending however the task
        // ends — normal return, a panic inside revive_entry, or the future
        // being dropped unpolled (runtime shutdown). Created BEFORE the spawn
        // so no path can leak the flag and leave the entry unrevivable.
        let guard = RevivePendingGuard { entry };
        crate::app::spawn_named("h2_pool_revive", async move {
            if pool.shutdown.load(Ordering::Relaxed) {
                return;
            }
            if pool.revive_entry(&guard.entry).await.is_ok() {
                pool.publish_alive_gauge();
            }
            // Err → dead stays; the cooldown gates the next attempt.
        });
    }

    /// Kick off a background task that fills the pool up to `pool_size`. Lets
    /// a low-RPS location warm to full instead of paying a connect on each of
    /// the first `pool_size` requests. Skips an **empty** pool — that means it
    /// was never used (or was drained), and creation must stay lazy. Guarded
    /// by `top_up_pending` so ticks don't stack top-up tasks.
    pub fn spawn_top_up(self: &Arc<Self>) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        let target = self.params.pool_size as usize;
        let total = self.total_count();
        if total == 0 || total >= target {
            return;
        }
        if self
            .top_up_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }

        let pool = self.clone();
        let guard = TopUpPendingGuard { pool: self.clone() };
        crate::app::spawn_named("h2_pool_top_up", async move {
            let _guard = guard;
            while !pool.shutdown.load(Ordering::Relaxed) && pool.total_count() < target {
                match pool.connect_one().await {
                    Ok(client) => {
                        let entry = Arc::new(H2Entry::new(Arc::new(client)));
                        // Lost the size race (Path 0 / another top-up filled
                        // it) — drop this one-shot and stop.
                        if !pool.try_push(entry) {
                            break;
                        }
                    }
                    // Upstream not reachable — stop; the next tick retries.
                    Err(_) => break,
                }
            }
            pool.publish_alive_gauge();
        });
    }
}

/// An invalid configured liveness path (request-line-forbidden bytes) must
/// never read as "upstream dead" — mirrors the h1 supervisor's guard.
fn liveness_path_is_valid(path: &str) -> bool {
    !path.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0 | b' '))
}

struct TopUpPendingGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pool: Arc<H2Pool<TStream, TConnector>>,
}

impl<TStream, TConnector> Drop for TopUpPendingGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn drop(&mut self) {
        self.pool.top_up_pending.store(false, Ordering::Release);
    }
}

struct RevivePendingGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    entry: Arc<H2Entry<TStream, TConnector>>,
}

impl<TStream, TConnector> Drop for RevivePendingGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn drop(&mut self) {
        self.entry.revive_pending.store(false, Ordering::Release);
    }
}

async fn ping_entry<TStream, TConnector>(
    entry: &Arc<H2Entry<TStream, TConnector>>,
    health_check_path: &str,
    authority: &str,
    ping_timeout: Duration,
) -> bool
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let path = if health_check_path.starts_with('/') {
        health_check_path.to_string()
    } else {
        format!("/{}", health_check_path)
    };
    let uri = format!("http://{}{}", authority, path);

    let req = match hyper::Request::builder()
        .method(hyper::Method::GET)
        .uri(uri)
        .body(Full::new(Bytes::new()))
    {
        Ok(r) => r,
        Err(_) => return false,
    };

    let client = entry.client.load_full();
    match tokio::time::timeout(ping_timeout, do_ping(&client, req, ping_timeout)).await {
        Ok(Ok(status)) => (200..=205).contains(&status),
        _ => false,
    }
}

async fn do_ping<TStream, TConnector>(
    client: &Arc<MyHttp2Client<TStream, TConnector>>,
    req: hyper::Request<Full<Bytes>>,
    ping_timeout: Duration,
) -> Result<u16, my_http_client::MyHttpClientError>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let resp = client.do_request(&req, ping_timeout).await?;
    Ok(resp.status().as_u16())
}
