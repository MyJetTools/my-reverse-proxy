use std::{collections::VecDeque, sync::Arc};

use ahash::AHashMap;
use parking_lot::Mutex;
use rust_extensions::date_time::DateTimeAsMicroseconds;

/// How many messages we keep per port / endpoint / location.
const MAX_LOG_MESSAGES: usize = 100;

#[derive(Clone)]
pub struct ProxyLogEntry {
    pub moment: DateTimeAsMicroseconds,
    pub message: String,
}

impl ProxyLogEntry {
    pub fn new(message: String) -> Self {
        Self {
            moment: DateTimeAsMicroseconds::now(),
            message,
        }
    }
}

/// A resolved (endpoint, location) pair carried into spawned tasks (websocket
/// pumps, ws loops) so their diagnostics land in the right in-memory buffers
/// without threading two separate parameters around.
#[derive(Clone)]
pub struct ProxyLogScope {
    pub endpoint: Arc<String>,
    pub location_id: i64,
}

impl ProxyLogScope {
    pub fn new(endpoint: Arc<String>, location_id: i64) -> Self {
        Self {
            endpoint,
            location_id,
        }
    }

    pub fn write(&self, message: String) {
        crate::app::APP_CTX
            .proxy_logs
            .write(&self.endpoint, Some(self.location_id), message);
    }
}

struct ProxyLogsInner {
    /// Pre-endpoint logs: a connection hit the listener but we could not resolve
    /// an endpoint (no config for the port, unknown SNI, Host header missing or
    /// not matching). Key is the listen identifier — TCP port as a decimal
    /// string, or the unix-socket path.
    by_port: AHashMap<String, VecDeque<ProxyLogEntry>>,
    /// Logs attributed to a resolved endpoint (`host_endpoint` string).
    by_endpoint: AHashMap<String, VecDeque<ProxyLogEntry>>,
    /// Logs attributed to a resolved location (`ProxyPassLocationConfig.id`).
    by_location: AHashMap<i64, VecDeque<ProxyLogEntry>>,
}

impl ProxyLogsInner {
    fn new() -> Self {
        Self {
            by_port: AHashMap::new(),
            by_endpoint: AHashMap::new(),
            by_location: AHashMap::new(),
        }
    }

    fn push(buf: &mut VecDeque<ProxyLogEntry>, message: String) {
        if buf.len() == MAX_LOG_MESSAGES {
            buf.pop_front();
        }
        buf.push_back(ProxyLogEntry::new(message));
    }

    fn push_into(map: &mut AHashMap<String, VecDeque<ProxyLogEntry>>, key: &str, message: String) {
        match map.get_mut(key) {
            Some(buf) => Self::push(buf, message),
            None => {
                let mut buf = VecDeque::new();
                Self::push(&mut buf, message);
                map.insert(key.to_string(), buf);
            }
        }
    }

    fn push_into_location(&mut self, location_id: i64, message: String) {
        match self.by_location.get_mut(&location_id) {
            Some(buf) => Self::push(buf, message),
            None => {
                let mut buf = VecDeque::new();
                Self::push(&mut buf, message);
                self.by_location.insert(location_id, buf);
            }
        }
    }
}

/// In-memory ring buffers (last 100 messages) for proxy diagnostics, grouped
/// along three axes that match the UI hierarchy: port → endpoint → location.
/// Mirrors the `Metrics` pattern: a `parking_lot::Mutex` with synchronous
/// methods (logging is on the hot path, nothing is awaited under the lock).
pub struct ProxyLogs {
    inner: Mutex<ProxyLogsInner>,
}

impl ProxyLogs {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ProxyLogsInner::new()),
        }
    }

    /// Pre-endpoint log: a connection hit the listener but no endpoint could be
    /// resolved. `listen_id` is the TCP port (as a decimal string) or the unix
    /// socket path.
    pub fn write_port(&self, listen_id: &str, message: String) {
        let mut inner = self.inner.lock();
        ProxyLogsInner::push_into(&mut inner.by_port, listen_id, message);
    }

    /// Log attributed to a resolved endpoint and, when known, the location.
    pub fn write(&self, endpoint: &str, location_id: Option<i64>, message: String) {
        let mut inner = self.inner.lock();
        ProxyLogsInner::push_into(&mut inner.by_endpoint, endpoint, message.clone());
        if let Some(location_id) = location_id {
            inner.push_into_location(location_id, message);
        }
    }

    /// Log attributed to a resolved location only (request-building / payload
    /// details, where the endpoint string is not in scope).
    pub fn write_location(&self, location_id: i64, message: String) {
        self.inner.lock().push_into_location(location_id, message);
    }

    pub fn get_by_port(&self, listen_id: &str) -> Vec<ProxyLogEntry> {
        get_snapshot(&self.inner.lock().by_port, listen_id)
    }

    pub fn get_by_endpoint(&self, endpoint: &str) -> Vec<ProxyLogEntry> {
        get_snapshot(&self.inner.lock().by_endpoint, endpoint)
    }

    pub fn get_by_location(&self, location_id: i64) -> Vec<ProxyLogEntry> {
        match self.inner.lock().by_location.get(&location_id) {
            Some(buf) => buf.iter().cloned().collect(),
            None => Vec::new(),
        }
    }
}

fn get_snapshot(map: &AHashMap<String, VecDeque<ProxyLogEntry>>, key: &str) -> Vec<ProxyLogEntry> {
    match map.get(key) {
        Some(buf) => buf.iter().cloned().collect(),
        None => Vec::new(),
    }
}
