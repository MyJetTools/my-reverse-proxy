use std::time::Duration;

#[derive(Clone, Debug)]
pub struct PoolParams {
    pub pool_size: u8,
    pub health_check_path: Option<String>,
    pub connect_timeout: Duration,
    pub ping_timeout: Duration,
    pub hot_window: Duration,
}

impl Default for PoolParams {
    fn default() -> Self {
        Self {
            pool_size: crate::consts::DEFAULT_POOL_SIZE,
            health_check_path: None,
            connect_timeout: crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
            ping_timeout: crate::consts::DEFAULT_POOL_PING_TIMEOUT,
            hot_window: crate::consts::DEFAULT_POOL_HOT_WINDOW,
        }
    }
}
