use std::time::Duration;

use crate::settings::LocationSettings;

/// Per-location tuning of the H1/H2 upstream connection pool internals,
/// resolved from the location config with fallback to the defaults in
/// `crate::consts`. Shared by both the h1 and h2 pools.
#[derive(Debug, Clone, Copy)]
pub struct PoolTuning {
    pub pool_size: u8,
    pub ping_timeout: Duration,
    pub hot_window: Duration,
}

impl PoolTuning {
    pub fn from_location_settings(location: &LocationSettings) -> Self {
        Self {
            pool_size: location.get_pool_size(),
            ping_timeout: location.get_pool_ping_timeout(),
            hot_window: location.get_pool_hot_window(),
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
