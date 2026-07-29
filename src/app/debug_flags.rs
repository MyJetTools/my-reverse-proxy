use ahash::AHashSet;
use parking_lot::RwLock;

/// Runtime, UI-toggled debug flags. When an endpoint or location is marked as
/// debug here, the proxy emits its verbose diagnostics into the in-memory
/// `ProxyLogs` buffers; otherwise nothing is captured for it. This replaces the
/// settings-driven `debug` / `trace_payload` flags as the gate for in-memory
/// logging — the toggles are flipped from the admin UI and are not persisted.
///
/// Reads happen on every request (`is_endpoint_debug` / `is_location_debug`),
/// writes only on a UI toggle, so a `parking_lot::RwLock` over small sets fits.
pub struct DebugFlags {
    inner: RwLock<DebugFlagsInner>,
}

struct DebugFlagsInner {
    endpoints: AHashSet<String>,
    locations: AHashSet<i64>,
}

impl DebugFlags {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(DebugFlagsInner {
                endpoints: AHashSet::new(),
                locations: AHashSet::new(),
            }),
        }
    }

    pub fn is_endpoint_debug(&self, endpoint: &str) -> bool {
        let inner = self.inner.read();
        if inner.endpoints.is_empty() {
            return false;
        }
        inner.endpoints.contains(endpoint)
    }

    pub fn is_location_debug(&self, location_id: i64) -> bool {
        let inner = self.inner.read();
        if inner.locations.is_empty() {
            return false;
        }
        inner.locations.contains(&location_id)
    }

    pub fn set_endpoint(&self, endpoint: &str, enabled: bool) {
        let mut inner = self.inner.write();
        if enabled {
            inner.endpoints.insert(endpoint.to_string());
        } else {
            inner.endpoints.remove(endpoint);
        }
    }

    pub fn set_location(&self, location_id: i64, enabled: bool) {
        let mut inner = self.inner.write();
        if enabled {
            inner.locations.insert(location_id);
        } else {
            inner.locations.remove(&location_id);
        }
    }
}
