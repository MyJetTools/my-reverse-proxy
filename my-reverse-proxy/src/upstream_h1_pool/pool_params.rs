use std::time::Duration;

#[derive(Clone, Debug)]
pub struct PoolParams {
    pub pool_size: u8,
    /// Reserved for symmetry with h2-pool. Active probe for h1 is not
    /// implemented — supervisor only refills empty slots.
    #[allow(dead_code)]
    pub health_check_path: Option<String>,
    pub connect_timeout: Duration,
    pub ping_timeout: Duration,
    pub hot_window: Duration,
    /// Per-read inactivity timeout passed to every connection this pool creates
    /// (`MyHttpClient::set_read_from_stream_timeout`). Large for MCP locations so
    /// idle SSE is not torn down.
    pub read_stream_timeout: Duration,
    /// Per-pool cap on concurrent on-demand (disposable) overflow connections.
    pub max_disposables: usize,
}

impl Default for PoolParams {
    fn default() -> Self {
        Self {
            pool_size: crate::consts::DEFAULT_POOL_SIZE,
            health_check_path: None,
            connect_timeout: crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
            ping_timeout: crate::consts::DEFAULT_POOL_PING_TIMEOUT,
            hot_window: crate::consts::DEFAULT_POOL_HOT_WINDOW,
            read_stream_timeout: crate::consts::DEFAULT_READ_TIMEOUT,
            max_disposables: crate::consts::DEFAULT_MAX_DISPOSABLES_PER_POOL,
        }
    }
}
