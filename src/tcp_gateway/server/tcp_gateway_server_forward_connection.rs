use std::sync::Arc;

use tokio::net::tcp::OwnedWriteHalf;

use crate::tcp_gateway::{TcpConnectionInner, TcpGatewayForwardConnection};

pub struct TcpGatewayServerForwardConnection {
    pub gateway_id: Arc<String>,
    pub remote_addr: Arc<String>,
    inner: Arc<TcpConnectionInner>,
}

impl TcpGatewayServerForwardConnection {
    pub fn new(
        gateway_id: Arc<String>,
        remote_addr: Arc<String>,
        connection: OwnedWriteHalf,
    ) -> Self {
        let (inner, receiver) = TcpConnectionInner::new(connection);
        let inner = Arc::new(inner);
        let result = Self {
            gateway_id,
            remote_addr,
            inner: inner.clone(),
        };

        super::super::tcp_connection_inner::start_write_loop(inner, receiver);

        result
    }
}

#[async_trait::async_trait]
impl TcpGatewayForwardConnection for TcpGatewayServerForwardConnection {
    fn get_addr(&self) -> &str {
        self.remote_addr.as_str()
    }
    async fn send_payload(&self, payload: &[u8]) -> bool {
        self.inner.send_payload(payload).await
    }

    async fn disconnect(&self) {
        self.inner.disconnect().await;
    }
}
