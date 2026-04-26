use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use my_http_client::{http1::MyHttpClient, MyHttpClientConnector, MyHttpClientError};

use super::{ConnectorFactory, H1ClientHandle, H1Slot, PoolKey, PoolParams};

pub struct H1Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub key: PoolKey,
    pub params: PoolParams,
    pub slots: Vec<Arc<H1Slot<TStream, TConnector>>>,
    pub next: AtomicUsize,
    pub shutdown: AtomicBool,
    pub factory: ConnectorFactory<TConnector>,
}

impl<TStream, TConnector> H1Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new(
        key: PoolKey,
        params: PoolParams,
        factory: ConnectorFactory<TConnector>,
    ) -> Self {
        let slots = (0..params.pool_size as usize)
            .map(|_| Arc::new(H1Slot::new()))
            .collect();
        Self {
            key,
            params,
            slots,
            next: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            factory,
        }
    }

    /// Round-robin pick of a non-rented, populated slot. Acquires the slot by
    /// flipping `rented` from false to true atomically. Returns None if every
    /// populated slot is currently rented or every slot is empty.
    pub fn get_connection(&self) -> Option<H1ClientHandle<TStream, TConnector>> {
        let n = self.slots.len();
        if n == 0 {
            return None;
        }
        for _ in 0..n {
            let idx = self.next.fetch_add(1, Ordering::Relaxed) % n;
            let slot = &self.slots[idx];
            let Some(client) = slot.client.load_full() else {
                continue;
            };
            if slot
                .rented
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return Some(H1ClientHandle::reusable(client, slot.clone()));
            }
        }
        None
    }

    /// Fresh client created via the same factory the supervisor uses, but not
    /// stored in any slot — caller owns its lifetime via the returned handle.
    pub async fn create_connection(
        &self,
    ) -> Result<H1ClientHandle<TStream, TConnector>, MyHttpClientError> {
        let (connector, metrics) = (self.factory)();
        let mut client = MyHttpClient::new_with_metrics(connector, metrics);
        client.set_connect_timeout(self.params.connect_timeout);
        let client = Arc::new(client);
        client.connect().await?;
        Ok(H1ClientHandle::disposable(client))
    }

    pub fn ready_slots(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.client.load().is_some())
            .count()
    }

    /// One supervisor pass: refill empty slots. Driven externally by a MyTimer
    /// tick (panic-safe — the timer keeps ticking even if a tick panics).
    pub async fn supervisor_tick(&self) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }

        let label = self.key.endpoint_label();
        for slot in self.slots.iter() {
            if self.shutdown.load(Ordering::Relaxed) {
                return;
            }
            if slot.client.load().is_some() {
                continue;
            }
            let (connector, metrics) = (self.factory)();
            let mut client = MyHttpClient::new_with_metrics(connector, metrics);
            client.set_connect_timeout(self.params.connect_timeout);
            let client_arc = Arc::new(client);
            if client_arc.connect().await.is_ok() {
                slot.client.store(Some(client_arc));
                slot.fail_count.store(0, Ordering::Relaxed);
                crate::app::APP_CTX.prometheus.inc_h1_pool_alive(&label);
            }
        }
    }
}
