use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use parking_lot::Mutex;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use super::OwnedUpstream;

/// A kept connection plus the moment it was parked, so the GC can tell how long
/// it has sat unused.
struct Kept {
    owned: OwnedUpstream,
    idle_since: DateTimeAsMicroseconds,
}

/// The one upstream connection an MCP client TCP keeps across its requests.
///
/// MCP does not go through [`super::H1PoolHolder`]. By the transport's own model
/// a client connection talks to exactly one MCP endpoint, so there is nothing to
/// key on and nothing to share — there is one connection, or none. That makes the
/// pool's whole apparatus (keys, idle sets, capacity) dead weight here, and its
/// key collision harmful: `connection_key` maps `Http1` and `McpHttp1` to the
/// same string, so a plain http location to the same host:port could otherwise
/// hand its idle socket to an MCP request.
///
/// The slot is emptied by [`take`](Self::take) for the duration of a request and
/// refilled by [`put`](Self::put) only when the response completed cleanly on a
/// still-live socket. Anything else simply never comes back, so the next request
/// finds `None` and dials fresh. That is the invalidation rule.
///
/// On top of that, a kept connection is not held forever: an MCP client may keep
/// its client TCP open for hours between requests with no heartbeat, and the
/// parked upstream socket would go stale (or just waste a file descriptor on both
/// ends) long before then. Each keeping connection registers itself with
/// [`crate::app::APP_CTX`]'s [`McpUpstreamRegistry`] on first use; the connections
/// GC timer sweeps them and closes any left idle past
/// [`crate::consts::DEFAULT_MCP_IDLE_TIMEOUT`].
pub struct McpUpstream {
    slot: Mutex<Option<Kept>>,
    /// Set once, the first time this keeps a connection, when it registers with
    /// the global registry so the GC can reach it.
    registered: AtomicBool,
}

impl McpUpstream {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            slot: Mutex::new(None),
            registered: AtomicBool::new(false),
        })
    }

    /// Check the kept connection out, leaving the slot empty. `None` means the
    /// caller must dial a fresh one — because there was nothing kept, or the kept
    /// one had died while parked, or it had sat idle past the timeout and must not
    /// be reused (the GC may not have swept it yet). A rejected connection drops
    /// here, which closes it.
    pub fn take(&self) -> Option<OwnedUpstream> {
        let kept = self.slot.lock().take()?;
        if kept.owned.upstream.is_disconnected() {
            return None;
        }
        if idle_micros(&kept) >= idle_timeout_micros() {
            return None;
        }
        Some(kept.owned)
    }

    /// Keep a connection for the next request on this client TCP, stamping the
    /// moment so the GC can age it out. A disconnected one is dropped (closed)
    /// instead. The first kept connection registers this slot with the global
    /// registry.
    pub fn put(self: &Arc<Self>, owned: OwnedUpstream) {
        if owned.upstream.is_disconnected() {
            return;
        }
        if !self.registered.swap(true, Ordering::Relaxed) {
            crate::app::APP_CTX.mcp_upstreams.register(self);
        }
        *self.slot.lock() = Some(Kept {
            owned,
            idle_since: DateTimeAsMicroseconds::now(),
        });
    }

    /// If a connection is parked here and has been idle past the timeout, take it
    /// out and hand it back for the caller to drop (closing it) OUTSIDE any lock.
    /// Returns `None` when there is nothing to close.
    fn evict_if_idle(&self, now: DateTimeAsMicroseconds) -> Option<OwnedUpstream> {
        let mut slot = self.slot.lock();
        let over_age = match slot.as_ref() {
            Some(kept) => {
                now.unix_microseconds - kept.idle_since.unix_microseconds >= idle_timeout_micros()
            }
            None => false,
        };
        if over_age {
            return slot.take().map(|kept| kept.owned);
        }
        None
    }
}

fn idle_timeout_micros() -> i64 {
    crate::consts::DEFAULT_MCP_IDLE_TIMEOUT.as_micros() as i64
}

fn idle_micros(kept: &Kept) -> i64 {
    DateTimeAsMicroseconds::now().unix_microseconds - kept.idle_since.unix_microseconds
}

/// Process-wide set of every live [`McpUpstream`] slot that currently keeps (or
/// has kept) a connection. Held by [`crate::app::APP_CTX`] and swept by the
/// connections GC timer. Entries are `Weak`: a client connection's slot lives on
/// its own reader task, so when that connection ends the strong refs go and the
/// GC prunes the dead `Weak` on its next pass.
pub struct McpUpstreamRegistry {
    entries: Mutex<Vec<Weak<McpUpstream>>>,
}

impl McpUpstreamRegistry {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    fn register(&self, mcp: &Arc<McpUpstream>) {
        self.entries.lock().push(Arc::downgrade(mcp));
    }

    /// Close every kept connection left idle past the timeout, and drop entries
    /// whose client connection has ended. Called from the connections GC timer.
    /// The actual socket closes happen after the registry lock is released, so a
    /// large sweep never holds the lock across the close syscalls.
    pub fn gc(&self) {
        let now = DateTimeAsMicroseconds::now();
        let mut to_close: Vec<OwnedUpstream> = Vec::new();
        {
            let mut entries = self.entries.lock();
            entries.retain(|weak| match weak.upgrade() {
                Some(mcp) => {
                    if let Some(owned) = mcp.evict_if_idle(now) {
                        to_close.push(owned);
                    }
                    true
                }
                None => false,
            });
        }
        drop(to_close);
    }
}
