use std::sync::Arc;

use parking_lot::Mutex;

use super::OwnedUpstream;

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
/// finds `None` and dials fresh. That is the entire invalidation rule: a
/// connection the worker is not sure about is not returned.
pub struct McpUpstream {
    slot: Mutex<Option<OwnedUpstream>>,
}

impl McpUpstream {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            slot: Mutex::new(None),
        })
    }

    /// Check the kept connection out, leaving the slot empty. `None` means the
    /// caller must dial a fresh one.
    ///
    /// A connection can die while parked here — nobody is reading it, so the
    /// disconnect flag may only have flipped on the last probe — hence the
    /// liveness check on the way out.
    pub fn take(&self) -> Option<OwnedUpstream> {
        let owned = self.slot.lock().take()?;
        if owned.upstream.is_disconnected() {
            return None;
        }
        Some(owned)
    }

    /// Keep a connection for the next request on this client TCP. A disconnected
    /// one is dropped (closed) instead.
    pub fn put(&self, owned: OwnedUpstream) {
        if owned.upstream.is_disconnected() {
            return;
        }
        *self.slot.lock() = Some(owned);
    }
}
