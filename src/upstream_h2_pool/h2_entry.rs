use std::sync::{atomic::AtomicBool, Arc};

use arc_swap::ArcSwap;
use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector};
use rust_extensions::date_time::AtomicDateTimeAsMicroseconds;

pub struct H2Entry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub client: ArcSwap<MyHttp2Client<TStream, TConnector>>,
    pub dead: AtomicBool,
    /// Refreshed on every successful do_request. Tick uses this to skip
    /// pinging "hot" entries.
    pub last_success: AtomicDateTimeAsMicroseconds,
    /// Per-entry async lock — serializes revival across both foreground
    /// (get_connection Path B) and background (supervisor revive_task).
    /// Path A (live hot pick) is lock-free and never touches this.
    pub revive_lock: tokio::sync::Mutex<()>,
}

impl<TStream, TConnector> H2Entry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new(client: Arc<MyHttp2Client<TStream, TConnector>>) -> Self {
        Self {
            client: ArcSwap::new(client),
            dead: AtomicBool::new(false),
            last_success: AtomicDateTimeAsMicroseconds::now(),
            revive_lock: tokio::sync::Mutex::new(()),
        }
    }
}
