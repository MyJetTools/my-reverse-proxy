use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use my_http_client::{
    http2::MyHttp2Client, hyper::MyHttpHyperClientMetrics, MyHttpClientConnector,
};

use crate::app::APP_CTX;

use super::{H2Pool, H2Scheme, H2Slot, PoolKey};

const PING_TIMEOUT: Duration = Duration::from_secs(1);
const FAIL_THRESHOLD: u8 = 3;

pub type ConnectorFactory<TConnector> = Arc<
    dyn Fn() -> (
            TConnector,
            Arc<dyn MyHttpHyperClientMetrics + Send + Sync + 'static>,
        ) + Send
        + Sync
        + 'static,
>;

pub fn spawn_supervisor<TStream, TConnector>(pool: Arc<H2Pool<TStream, TConnector>>)
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    tokio::spawn(supervisor_loop(pool));
}

async fn supervisor_loop<TStream, TConnector>(pool: Arc<H2Pool<TStream, TConnector>>)
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    let interval = pool.params.health_check_interval;
    let label = pool.key.endpoint_label();

    loop {
        if pool.shutdown.load(Ordering::Relaxed) {
            return;
        }

        for slot in pool.slots.iter() {
            if pool.shutdown.load(Ordering::Relaxed) {
                return;
            }

            let current = slot.client.load_full();
            match current {
                None => {
                    let (connector, metrics) = (pool.factory)();
                    let mut client = MyHttp2Client::new_with_metrics(connector, metrics);
                    client.set_connect_timeout(pool.params.connect_timeout);
                    let client_arc = Arc::new(client);
                    if client_arc.connect().await.is_ok() {
                        slot.client.store(Some(client_arc));
                        slot.fail_count.store(0, Ordering::Relaxed);
                        APP_CTX.prometheus.inc_h2_pool_alive(&label);
                    }
                }
                Some(client) => {
                    if let Some(path) = pool.params.health_check_path.as_deref() {
                        ping_slot(slot, &client, path, &pool.key, &label).await;
                    }
                }
            }
        }

        tokio::time::sleep(interval).await;
    }
}

async fn ping_slot<TStream, TConnector>(
    slot: &Arc<H2Slot<TStream, TConnector>>,
    client: &Arc<MyHttp2Client<TStream, TConnector>>,
    health_check_path: &str,
    key: &PoolKey,
    label: &str,
) where
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
        Err(_) => return,
    };

    let ok = match tokio::time::timeout(PING_TIMEOUT, client.do_request(req, PING_TIMEOUT)).await {
        Ok(Ok(resp)) => {
            let s = resp.status().as_u16();
            (200..=205).contains(&s)
        }
        _ => false,
    };

    if ok {
        slot.fail_count.store(0, Ordering::Relaxed);
    } else {
        let n = slot.fail_count.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= FAIL_THRESHOLD {
            if slot.client.swap(None).is_some() {
                APP_CTX.prometheus.dec_h2_pool_alive(label);
            }
            slot.fail_count.store(0, Ordering::Relaxed);
        }
    }
}
