use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use my_http_client::{
    http2::MyHttp2Client, hyper::MyHttpHyperClientMetrics, MyHttpClientConnector,
};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::app::APP_CTX;

use super::{H2Entry, H2Pool, H2Scheme, PoolKey};

const PING_TIMEOUT: Duration = Duration::from_secs(1);
const HOT_WINDOW_SECS: i64 = 3;

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
            .set_h2_pool_alive(&label, self.alive_count() as i64);
    }
}

fn spawn_revive<TStream, TConnector>(
    pool: Arc<H2Pool<TStream, TConnector>>,
    dead_entry: Arc<H2Entry<TStream, TConnector>>,
) where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    tokio::spawn(async move {
        if pool.shutdown.load(Ordering::Relaxed) {
            return;
        }
        if pool.revive_entry(&dead_entry).await.is_ok() {
            APP_CTX
                .prometheus
                .set_h2_pool_alive(&pool.key.endpoint_label(), pool.alive_count() as i64);
        }
        // Err → dead stays; next tick will spawn another revive task.
    });
}

async fn ping_entry<TStream, TConnector>(
    entry: &Arc<H2Entry<TStream, TConnector>>,
    health_check_path: &str,
    key: &PoolKey,
) -> bool
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let authority = match key.scheme {
        H2Scheme::UnixHttp2 => "localhost".to_string(),
        H2Scheme::Http2 | H2Scheme::Https2 => format!("{}:{}", key.host, key.port),
    };
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
    match tokio::time::timeout(PING_TIMEOUT, do_ping(&client, req)).await {
        Ok(Ok(status)) => (200..=205).contains(&status),
        _ => false,
    }
}

async fn do_ping<TStream, TConnector>(
    client: &Arc<MyHttp2Client<TStream, TConnector>>,
    req: hyper::Request<Full<Bytes>>,
) -> Result<u16, my_http_client::MyHttpClientError>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let resp = client.do_request(req, PING_TIMEOUT).await?;
    Ok(resp.status().as_u16())
}
