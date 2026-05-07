use std::sync::{atomic::AtomicBool, Arc};

use arc_swap::ArcSwap;
use my_http_client::{http1::MyHttpClient, MyHttpClientConnector};
use rust_extensions::date_time::AtomicDateTimeAsMicroseconds;

pub struct H1Entry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub client: ArcSwap<MyHttpClient<TStream, TConnector>>,
    pub dead: AtomicBool,
    /// Refreshed on every successful do_request. Tick uses this to skip
    /// pinging "hot" entries.
    pub last_success: AtomicDateTimeAsMicroseconds,
    /// `true` while a request is in-flight on this client. h1 is single-stream,
    /// so the pool hands out at most one concurrent rent per entry.
    pub rented: AtomicBool,
    /// Per-entry async lock — serializes revival across foreground (Path B in
    /// get_connection) and background (supervisor revive_task). Path A is
    /// lock-free.
    pub revive_lock: tokio::sync::Mutex<()>,
}

impl<TStream, TConnector> H1Entry<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new(client: Arc<MyHttpClient<TStream, TConnector>>) -> Self {
        Self {
            client: ArcSwap::new(client),
            dead: AtomicBool::new(false),
            last_success: AtomicDateTimeAsMicroseconds::now(),
            rented: AtomicBool::new(false),
            revive_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Returns true if we successfully marked it as rented (was free before).
    pub fn try_rent(&self) -> bool {
        self.rented
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
    }

    pub fn release_rent(&self) {
        self.rented
            .store(false, std::sync::atomic::Ordering::Release);
    }
}
