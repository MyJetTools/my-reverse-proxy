use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use http_body_util::BodyExt;
use my_http_client::{
    http1::{MyHttpClient, MyHttpClientMetrics},
    MyHttpClientConnector,
};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::upstream_status::UpstreamStatus;

use super::{H1Entry, H1Pool};

pub type ConnectorFactory<TConnector> = Arc<
    dyn Fn() -> (
            TConnector,
            Arc<dyn MyHttpClientMetrics + Send + Sync + 'static>,
        ) + Send
        + Sync
        + 'static,
>;

pub const MAX_DISPOSABLE: usize = 100;
pub static DISPOSABLE_COUNTER: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

impl<TStream, TConnector> H1Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    /// One supervisor pass:
    /// - dead → spawn a background revive task (uses entry.revive_lock).
    /// - !dead AND `now - last_success < hot_window` → skip (hot, no probe).
    /// - !dead AND idle AND `health_check_path` set → rent + ping. Fail →
    ///   mark dead + spawn revive. Busy (rented) entries are skipped: h1 is
    ///   single-stream, and an unrented ping would pipeline a second request
    ///   onto a connection that is serving one — crossing the responses.
    pub async fn supervisor_tick(self: &Arc<Self>) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }

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

            let idle = now
                .duration_since(entry.last_success.as_date_time())
                .as_positive_or_zero();
            if idle < self.params.hot_window {
                continue;
            }

            let Some(path) = self.params.health_check_path.as_deref() else {
                continue;
            };
            // A malformed configured path would panic in MyHttpRequestBuilder
            // (request-line-forbidden bytes) — a config typo must not take
            // down supervision or wedge a rent. Skip probing instead.
            if !liveness_path_is_valid(path) {
                continue;
            }

            // Exclusive rent for the probe, exactly like a real request.
            // Rented == a request is in flight == the connection works;
            // nothing to probe (and probing it would corrupt the stream).
            if !entry.try_rent() {
                continue;
            }
            // The tick future can be dropped at any await (MyTimer cancels a
            // tick that exceeds its iteration timeout) — the rent must never
            // leak, or the entry is excluded from Phase 1 and probing forever.
            let rent = RentGuard {
                entry: entry.clone(),
            };
            let alive =
                ping_entry(entry, path, &self.desc.authority, self.params.ping_timeout).await;
            if alive {
                entry.last_success.update(DateTimeAsMicroseconds::now());
                self.last_status.set(UpstreamStatus::Ok);
            } else {
                entry.dead.store(true, Ordering::Relaxed);
                self.last_status.set(UpstreamStatus::Error);
            }
            drop(rent);
            if !alive {
                self.spawn_revive(entry.clone());
            }
        }

        self.publish_alive_gauge();
    }

    /// Kick off a background revive for a dead entry. Deduplicated via
    /// `revive_pending` (CAS) so ticks can't stack tasks for the same entry.
    /// No cooldown here, unlike h2: the h1 model requires the foreground
    /// Path B to always dial (a 503 must mean a dial actually failed), so the
    /// only background dial pacing is the tick cadence itself.
    pub fn spawn_revive(self: &Arc<Self>, entry: Arc<H1Entry<TStream, TConnector>>) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        if !entry.dead.load(Ordering::Relaxed) {
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
        // being dropped unpolled. Created BEFORE the spawn so no path leaks
        // the flag and leaves the entry unrevivable by the supervisor.
        let guard = RevivePendingGuard { entry };
        crate::app::spawn_named("h1_pool_revive", async move {
            if pool.shutdown.load(Ordering::Relaxed) {
                return;
            }
            if pool.revive_entry(&guard.entry).await.is_ok() {
                pool.publish_alive_gauge();
            }
            // Err → dead stays; next tick will spawn another revive task.
        });
    }
}

/// Releases the probe's rent on every exit path — normal flow, a panic inside
/// the ping, or the tick future being dropped mid-await. A leaked rent would
/// exclude the entry from Phase 1 and from probing forever.
struct RentGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    entry: Arc<H1Entry<TStream, TConnector>>,
}

impl<TStream, TConnector> Drop for RentGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn drop(&mut self) {
        self.entry.release_rent();
    }
}

/// The raw h1 request builder panics on request-line-forbidden bytes; a config
/// typo in the liveness path must not panic the supervisor.
fn liveness_path_is_valid(path: &str) -> bool {
    !path.bytes().any(|b| matches!(b, b'\r' | b'\n' | 0 | b' '))
}

struct RevivePendingGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    entry: Arc<H1Entry<TStream, TConnector>>,
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
    entry: &Arc<H1Entry<TStream, TConnector>>,
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

    // HTTP/1.1 REQUIRES a Host header — a compliant upstream answers 400
    // without it, which would read as "dead" and churn a healthy connection.
    let mut builder = my_http_client::http1::MyHttpRequestBuilder::new(hyper::Method::GET, &path);
    builder.append_header("Host", authority);
    let req = builder.build();
    let client = entry.client.load_full();
    match tokio::time::timeout(ping_timeout, do_ping(&client, &req, ping_timeout)).await {
        Ok(Ok(status)) => (200..=205).contains(&status),
        _ => false,
    }
}

async fn do_ping<TStream, TConnector>(
    client: &Arc<MyHttpClient<TStream, TConnector>>,
    req: &my_http_client::http1::MyHttpRequest,
    ping_timeout: Duration,
) -> Result<u16, my_http_client::MyHttpClientError>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let resp = client.do_request(req, ping_timeout).await?;
    match resp {
        my_http_client::http1::MyHttpResponse::Response(r) => {
            let status = r.status().as_u16();
            // Drain the body before dropping it: the h1 read loop streams
            // body frames into this response's channel, and a dropped
            // receiver fails the read loop and tears down the healthy
            // connection. The caller's ping_timeout bounds the whole probe.
            let mut body = r.into_body();
            while let Some(frame) = body.frame().await {
                if frame.is_err() {
                    break;
                }
            }
            Ok(status)
        }
        my_http_client::http1::MyHttpResponse::WebSocketUpgrade { .. } => Ok(0),
    }
}
