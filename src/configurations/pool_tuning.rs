use std::time::Duration;

use crate::settings::ResolvedTimeouts;

/// Per-location tuning of the H1/H2 upstream connection pool internals, taken
/// from the resolved timeout cascade. Shared by both the h1 and h2 pools.
#[derive(Debug, Clone, Copy)]
pub struct PoolTuning {
    pub pool_size: u8,
    pub ping_timeout: Duration,
    pub hot_window: Duration,
}

impl PoolTuning {
    pub fn from_resolved(resolved: &ResolvedTimeouts) -> Self {
        Self {
            pool_size: resolved.pool_size,
            ping_timeout: resolved.pool_ping_timeout,
            hot_window: resolved.pool_hot_window,
        }
    }
}

impl Default for PoolTuning {
    fn default() -> Self {
        Self {
            pool_size: crate::consts::DEFAULT_POOL_SIZE,
            ping_timeout: crate::consts::DEFAULT_POOL_PING_TIMEOUT,
            hot_window: crate::consts::DEFAULT_POOL_HOT_WINDOW,
        }
    }
}
