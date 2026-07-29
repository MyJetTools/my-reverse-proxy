use std::time::Duration;

/// Transport-level idle read/write timeouts for a connection's byte pumps.
/// Resolved per-endpoint from the timeout cascade (see `ResolvedTimeouts`); the
/// `Default` falls back to the hardcoded defaults.
#[derive(Clone, Copy)]
pub struct HttpTimeouts {
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for HttpTimeouts {
    fn default() -> Self {
        Self {
            read_timeout: crate::consts::DEFAULT_READ_TIMEOUT,
            write_timeout: crate::consts::DEFAULT_WRITE_TIMEOUT,
        }
    }
}
