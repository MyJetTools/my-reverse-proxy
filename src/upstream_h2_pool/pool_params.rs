use std::time::Duration;

#[derive(Clone, Debug)]
pub struct PoolParams {
    pub pool_size: u8,
    pub health_check_path: Option<String>,
    pub connect_timeout: Duration,
    pub ping_timeout: Duration,
    pub hot_window: Duration,
    /// Minimum spacing between reconnect attempts to a dead entry; attempts
    /// inside the window fail fast instead of re-dialing.
    pub revive_cooldown: Duration,
    /// Max time a request waits for an in-flight revive when every entry is
    /// dead, before failing fast.
    pub dead_pool_wait_budget: Duration,
    /// h2 keep-alive PING cadence / answer deadline for every connection this
    /// pool creates. Detects a dead transport while idle, before any request
    /// touches it.
    pub keep_alive_interval: Duration,
    pub keep_alive_timeout: Duration,
}

impl Default for PoolParams {
    fn default() -> Self {
        Self {
            pool_size: crate::consts::DEFAULT_POOL_SIZE,
            health_check_path: None,
            connect_timeout: crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
            ping_timeout: crate::consts::DEFAULT_POOL_PING_TIMEOUT,
            hot_window: crate::consts::DEFAULT_POOL_HOT_WINDOW,
            revive_cooldown: crate::consts::DEFAULT_POOL_REVIVE_COOLDOWN,
            dead_pool_wait_budget: crate::consts::DEFAULT_POOL_DEAD_POOL_WAIT_BUDGET,
            keep_alive_interval: crate::consts::DEFAULT_H2_KEEP_ALIVE_INTERVAL,
            keep_alive_timeout: crate::consts::DEFAULT_H2_KEEP_ALIVE_TIMEOUT,
        }
    }
}
