use std::collections::HashMap;

use parking_lot::Mutex;

#[derive(Default)]
pub struct MetricsValues<TKey: Clone + std::cmp::Eq + std::hash::Hash> {
    data: HashMap<TKey, isize>,
}

impl<TKey: Clone + std::cmp::Eq + std::hash::Hash> MetricsValues<TKey> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn inc(&mut self, key: &TKey) {
        if let Some(value) = self.data.get_mut(key) {
            *value += 1;
            return;
        }

        self.data.insert(key.clone(), 1);
    }

    pub fn dec(&mut self, key: &TKey) {
        if let Some(value) = self.data.get_mut(key) {
            *value -= 1;
            return;
        }

        self.data.insert(key.clone(), -1);
    }

    pub fn get(&self, key: &TKey) -> isize {
        match self.data.get(key) {
            Some(value) => *value,
            None => 0,
        }
    }
}

pub struct MetricsInner {
    pub connection_by_port: MetricsValues<u16>,
    /// Live inbound TCP connections attributed to a specific configured
    /// endpoint (host_endpoint string, e.g. `"myapp.com:443"`). For HTTPS
    /// the attribution happens after the TLS handshake (we know SNI →
    /// endpoint). For plain HTTP we attribute on first request once the
    /// endpoint is resolved from the Host header.
    pub connection_by_endpoint: MetricsValues<String>,
}

impl MetricsInner {
    pub fn new() -> Self {
        Self {
            connection_by_port: MetricsValues::new(),
            connection_by_endpoint: MetricsValues::new(),
        }
    }
}

pub struct Metrics {
    pub inner: Mutex<MetricsInner>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MetricsInner::new()),
        }
    }

    pub fn update(&self, access: impl Fn(&mut MetricsInner)) {
        let mut inner = self.inner.lock();
        access(&mut inner);
    }

    pub fn get<TResult>(&self, access: impl Fn(&MetricsInner) -> TResult) -> TResult {
        let inner = self.inner.lock();
        access(&inner)
    }
}
