use std::time::Duration;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

pub const DEFAULT_HTTP_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub const READ_TIMEOUT: Duration = Duration::from_secs(60 * 5);
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

// MCP endpoint defaults (used when not overridden in the endpoint config).
// The client→server direction of a tunneled MCP session can legitimately stay
// idle for long periods, so the read timeout is a generous safety net.
pub const DEFAULT_MCP_READ_TIMEOUT: Duration = Duration::from_secs(60 * 30);
pub const DEFAULT_MCP_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_MCP_BUFFER_SIZE: usize = 512 * 1024;

// H1/H2 upstream pool defaults (used when not overridden in the location config).
pub const DEFAULT_POOL_SIZE: u8 = 5;
// How long a connection is considered "hot" after its last successful use —
// within this window the supervisor skips the liveness probe.
pub const DEFAULT_POOL_HOT_WINDOW: Duration = Duration::from_secs(3);
// Per-probe timeout for the supervisor's liveness ping.
pub const DEFAULT_POOL_PING_TIMEOUT: Duration = Duration::from_secs(1);
// How often the (single, global) supervisor sweeps every pool.
pub const DEFAULT_POOL_SUPERVISOR_INTERVAL: Duration = Duration::from_secs(10);

pub const HTTP_CR_LF: &[u8] = b"\r\n";

pub const AUTHORIZED_COOKIE_NAME: &str = "x-authorized";

pub mod location_type {

    pub const MCP: &'static str = "mcp";
    pub const STATIC: &'static str = "static";
    pub const DYNAMIC: &'static str = "dynamic";
}
