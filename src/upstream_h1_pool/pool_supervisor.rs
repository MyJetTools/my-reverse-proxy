use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use my_http_client::{
    http1::{MyHttpClient, MyHttpClientMetrics},
    MyHttpClientConnector,
};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::app::APP_CTX;

use super::{H1Entry, H1Pool, H1Scheme, PoolKey};

const PING_TIMEOUT: Duration = Duration::from_secs(1);
const HOT_WINDOW_SECS: i64 = 3;

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
    /// - !dead AND `now - last_success < 3s` → skip (hot, no probe needed).
    /// - !dead AND idle AND `health_check_path` set → ping. Fail → mark dead +
    ///   spawn revive.
    pub async fn supervisor_tick(self: &Arc<Self>) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }

        let label = self.key.endpoint_label();
        let snap = self.clients.load_full();
        let now = DateTimeAsMicroseconds::now();

        for entry in snap.iter() {
            if self.shutdown.load(Ordering::Relaxed) {
                return;
            }

            if entry.dead.load(Ordering::Relaxed) {
                spawn_revive(self.clone(), entry.clone());
                continue;
            }

            let idle_secs = now
                .duration_since(entry.last_success.as_date_time())
                .as_positive_or_zero()
                .as_secs();
            if (idle_secs as i64) < HOT_WINDOW_SECS {
                continue;
            }

            let Some(path) = self.params.health_check_path.as_deref() else {
                continue;
            };

            let alive = ping_entry(entry, path, &self.key).await;
            if alive {
                entry.last_success.update(DateTimeAsMicroseconds::now());
            } else {
                entry.dead.store(true, Ordering::Relaxed);
                spawn_revive(self.clone(), entry.clone());
            }
        }

        APP_CTX
            .prometheus
            .set_h1_pool_alive(&label, self.alive_count() as i64);
    }
}

fn spawn_revive<TStream, TConnector>(
    pool: Arc<H1Pool<TStream, TConnector>>,
    dead_entry: Arc<H1Entry<TStream, TConnector>>,
) where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    crate::app::spawn_named("h1_pool_revive", async move {
        if pool.shutdown.load(Ordering::Relaxed) {
            return;
        }
        if pool.revive_entry(&dead_entry).await.is_ok() {
            APP_CTX
                .prometheus
                .set_h1_pool_alive(&pool.key.endpoint_label(), pool.alive_count() as i64);
        }
        // Err → dead stays; next tick will spawn another revive task.
    });
}

async fn ping_entry<TStream, TConnector>(
    entry: &Arc<H1Entry<TStream, TConnector>>,
    health_check_path: &str,
    _key: &PoolKey,
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

    let req = my_http_client::http1::MyHttpRequestBuilder::new(hyper::Method::GET, &path).build();
    let client = entry.client.load_full();
    match tokio::time::timeout(PING_TIMEOUT, do_ping(&client, &req)).await {
        Ok(Ok(status)) => (200..=205).contains(&status),
        _ => false,
    }
}

async fn do_ping<TStream, TConnector>(
    client: &Arc<MyHttpClient<TStream, TConnector>>,
    req: &my_http_client::http1::MyHttpRequest,
) -> Result<u16, my_http_client::MyHttpClientError>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let resp = client.do_request(req, PING_TIMEOUT).await?;
    match resp {
        my_http_client::http1::MyHttpResponse::Response(r) => Ok(r.status().as_u16()),
        my_http_client::http1::MyHttpResponse::WebSocketUpgrade { .. } => Ok(0),
    }
}

// Suppress unused warning for H1Scheme — used by other modules but not by this file.
#[allow(dead_code)]
fn _scheme_marker(_: H1Scheme) {}
