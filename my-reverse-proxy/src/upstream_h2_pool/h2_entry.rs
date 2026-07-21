use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

use arc_swap::ArcSwap;
use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector};
use rust_extensions::date_time::{
    AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds, DateTimeDuration,
};

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
    /// True while a background revive task is in flight for this entry —
    /// `spawn_revive` uses it to skip duplicate spawns.
    pub revive_pending: AtomicBool,
    /// Start of the most recent revive connect attempt. Attempts within
    /// `PoolParams::revive_cooldown` of it fail fast instead of re-dialing.
    pub last_revive_attempt: AtomicDateTimeAsMicroseconds,
    /// Per-entry async lock — serializes revival across both foreground
    /// (all-dead recovery in get_connection) and background (spawn_revive).
    /// The live pick path is lock-free and never touches this.
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
            revive_pending: AtomicBool::new(false),
            // Epoch — a fresh entry must never start inside the cooldown
            // window: the first revive dial is always allowed.
            last_revive_attempt: AtomicDateTimeAsMicroseconds::new(0),
            revive_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Remaining revive cooldown, or `None` when a dial is allowed. A stamp
    /// in the future (wall clock stepped backwards) counts as expired —
    /// revival must never freeze on clock jumps.
    pub fn revive_cooldown_remaining(&self, cooldown: Duration) -> Option<Duration> {
        match DateTimeAsMicroseconds::now().duration_since(self.last_revive_attempt.as_date_time())
        {
            DateTimeDuration::Positive(elapsed) => {
                if elapsed < cooldown {
                    Some(cooldown - elapsed)
                } else {
                    None
                }
            }
            DateTimeDuration::Zero => Some(cooldown),
            DateTimeDuration::Negative(_) => None,
        }
    }
}
