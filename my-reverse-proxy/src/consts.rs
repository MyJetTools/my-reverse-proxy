use std::time::Duration;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

pub const DEFAULT_HTTP_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

// Transport-level idle read/write timeouts, applied to every byte-pump path
// (H1 request/response transfer, MCP tunnel, TCP port-forward). Endpoint-scoped
// and overridable via the timeout cascade (global → endpoint). The read timeout
// doubles as dead-connection detection: if the peer drops, the pump unblocks
// after this long. 3 minutes balances that against legitimately-idle sessions
// (e.g. an MCP client that only speaks when it issues a request).
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60 * 3);
pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
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
// Minimum spacing between REVIVE dials to a dead pool entry. Repeat revive
// attempts inside the window fail fast instead of re-dialing, so a down
// upstream costs at most one revive dial per window per entry. (Does not
// rate-limit Path 0 growth of a below-target pool or on-demand WS connects.)
pub const DEFAULT_POOL_REVIVE_COOLDOWN: Duration = Duration::from_millis(500);
// How long a request may wait for an in-flight revive (the entry's
// revive_lock) when EVERY entry of the pool is dead, before failing fast.
// Bounds the waiters only: the one request that wins the lock dials with the
// full connect_timeout — someone has to, and it recovers the pool the moment
// the upstream is back.
pub const DEFAULT_POOL_DEAD_POOL_WAIT_BUDGET: Duration = Duration::from_millis(500);
// h2 transport keep-alive: a PING frame every interval; no answer within the
// timeout closes the connection (flipping the client's is_alive), so a dead
// idle socket is detected before user traffic ever touches it.
pub const DEFAULT_H2_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(10);
pub const DEFAULT_H2_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(2);
// Per-pool cap on concurrent on-demand (disposable) overflow connections. Below
// the global MAX_DISPOSABLE ceiling so one saturated upstream cannot starve the
// process-wide budget for the others.
pub const DEFAULT_MAX_DISPOSABLES_PER_POOL: usize = 50;
// Read/stream inactivity timeout for MCP upstream connections. MCP streamable-
// HTTP / SSE bodies can idle far longer than DEFAULT_READ_TIMEOUT with no
// keepalive, so MCP locations get a large value to avoid dropping a healthy but
// idle stream. (TCP RST/FIN still surfaces immediately regardless of this.)
pub const DEFAULT_MCP_READ_TIMEOUT: Duration = Duration::from_secs(60 * 60);

// A kept (idle, between-request) MCP upstream connection is closed after this
// long with no request on it, instead of being held — possibly already dead —
// for the whole life of the client connection (an MCP client may keep its TCP
// open for hours with no heartbeat). Swept by the connections GC timer; the next
// request simply dials a fresh one.
pub const DEFAULT_MCP_IDLE_TIMEOUT: Duration = Duration::from_secs(60 * 5);

pub const HTTP_CR_LF: &[u8] = b"\r\n";

pub const AUTHORIZED_COOKIE_NAME: &str = "x-authorized";

pub mod location_type {

    pub const MCP: &'static str = "mcp";
    pub const STATIC: &'static str = "static";
    pub const DYNAMIC: &'static str = "dynamic";
}
