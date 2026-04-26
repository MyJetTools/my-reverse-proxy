use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector, MyHttpClientError};

use super::{ConnectorFactory, H2Slot, PoolKey, PoolParams};

pub struct H2Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub key: PoolKey,
    pub params: PoolParams,
    pub slots: Vec<Arc<H2Slot<TStream, TConnector>>>,
    pub next: AtomicUsize,
    pub shutdown: AtomicBool,
    pub factory: ConnectorFactory<TConnector>,
}

impl<TStream, TConnector> H2Pool<TStream, TConnector>
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
            .map(|_| Arc::new(H2Slot::new()))
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

    /// Round-robin pick from pre-warmed pool. None if every slot is empty.
    pub fn get_connection(&self) -> Option<Arc<MyHttp2Client<TStream, TConnector>>> {
        let n = self.slots.len();
        if n == 0 {
            return None;
        }
        for _ in 0..n {
            let idx = self.next.fetch_add(1, Ordering::Relaxed) % n;
            if let Some(client) = self.slots[idx].client.load_full() {
                return Some(client);
            }
        }
        None
    }

    /// Fresh client created via the same factory the supervisor uses, but not
    /// stored in any slot — caller owns its lifetime via the returned Arc.
    pub async fn create_connection(
        &self,
    ) -> Result<Arc<MyHttp2Client<TStream, TConnector>>, MyHttpClientError> {
        let (connector, metrics) = (self.factory)();
        let mut client = MyHttp2Client::new_with_metrics(connector, metrics);
        client.set_connect_timeout(self.params.connect_timeout);
        let client = Arc::new(client);
        client.connect().await?;
        Ok(client)
    }

    pub fn ready_slots(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.client.load().is_some())
            .count()
    }
}
