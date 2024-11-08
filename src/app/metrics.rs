use std::{collections::HashMap, net::SocketAddr};

use tokio::sync::Mutex;

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
    pub server_connections: MetricsValues<SocketAddr>,
}

impl MetricsInner {
    pub fn new() -> Self {
        Self {
            connection_by_port: MetricsValues::new(),
            server_connections: MetricsValues::new(),
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

    pub async fn update(&self, access: impl Fn(&mut MetricsInner)) {
        let mut inner = self.inner.lock().await;
        access(&mut inner);
    }

    pub async fn get<TResult>(&self, access: impl Fn(&MetricsInner) -> TResult) -> TResult {
        let mut inner = self.inner.lock().await;
        access(&mut inner)
    }
}
