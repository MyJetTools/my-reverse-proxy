use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector};

use super::{H2Slot, PoolKey, PoolParams};

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
}

impl<TStream, TConnector> H2Pool<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new(key: PoolKey, params: PoolParams) -> Self {
        let slots = (0..params.pool_size as usize)
            .map(|_| Arc::new(H2Slot::new()))
            .collect();
        Self {
            key,
            params,
            slots,
            next: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn acquire(&self) -> Option<Arc<MyHttp2Client<TStream, TConnector>>> {
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

    pub fn ready_slots(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.client.load().is_some())
            .count()
    }
}
