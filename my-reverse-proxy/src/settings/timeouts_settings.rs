use std::time::Duration;

use serde::*;

/// The cascading timeout set. The same fields can appear at three config
/// levels — `global_settings`, the listen `endpoint`, and a `location` — and
/// are layered in that order, each overriding the previous, on top of the
/// hardcoded defaults in `crate::consts`:
///
/// HardCode  <  Global Settings  <  Listen Endpoint  <  Location
///
/// Every field is optional milliseconds (except `pool_size`); an unset field at
/// one level simply lets the level below show through.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct TimeoutsSettings {
    pub connect_timeout: Option<u64>,
    pub request_timeout: Option<u64>,
    pub mcp_read_timeout: Option<u64>,
    pub mcp_write_timeout: Option<u64>,
    pub pool_size: Option<u8>,
    pub pool_ping_timeout: Option<u64>,
    pub pool_hot_window: Option<u64>,
}

impl TimeoutsSettings {
    /// Layers `higher` on top of `self`: any field set in `higher` wins, the
    /// rest fall through to `self`. Apply left-to-right for the cascade
    /// (`global.overriden_by(endpoint).overriden_by(location)`).
    pub fn overriden_by(&self, higher: &TimeoutsSettings) -> TimeoutsSettings {
        TimeoutsSettings {
            connect_timeout: higher.connect_timeout.or(self.connect_timeout),
            request_timeout: higher.request_timeout.or(self.request_timeout),
            mcp_read_timeout: higher.mcp_read_timeout.or(self.mcp_read_timeout),
            mcp_write_timeout: higher.mcp_write_timeout.or(self.mcp_write_timeout),
            pool_size: higher.pool_size.or(self.pool_size),
            pool_ping_timeout: higher.pool_ping_timeout.or(self.pool_ping_timeout),
            pool_hot_window: higher.pool_hot_window.or(self.pool_hot_window),
        }
    }

    /// Fills every still-unset field with its hardcoded default, yielding the
    /// concrete values the runtime uses.
    pub fn resolve(&self) -> ResolvedTimeouts {
        ResolvedTimeouts {
            connect_timeout: ms_or(self.connect_timeout, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT),
            request_timeout: ms_or(self.request_timeout, crate::consts::DEFAULT_HTTP_REQUEST_TIMEOUT),
            mcp_read_timeout: ms_or(self.mcp_read_timeout, crate::consts::DEFAULT_MCP_READ_TIMEOUT),
            mcp_write_timeout: ms_or(self.mcp_write_timeout, crate::consts::DEFAULT_MCP_WRITE_TIMEOUT),
            pool_size: self.pool_size.unwrap_or(crate::consts::DEFAULT_POOL_SIZE),
            pool_ping_timeout: ms_or(self.pool_ping_timeout, crate::consts::DEFAULT_POOL_PING_TIMEOUT),
            pool_hot_window: ms_or(self.pool_hot_window, crate::consts::DEFAULT_POOL_HOT_WINDOW),
        }
    }
}

fn ms_or(value: Option<u64>, default: Duration) -> Duration {
    match value {
        Some(value) => Duration::from_millis(value),
        None => default,
    }
}

/// Concrete, fully-resolved timeout values (cascade applied, defaults filled).
#[derive(Debug, Clone, Copy)]
pub struct ResolvedTimeouts {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub mcp_read_timeout: Duration,
    pub mcp_write_timeout: Duration,
    pub pool_size: u8,
    pub pool_ping_timeout: Duration,
    pub pool_hot_window: Duration,
}
