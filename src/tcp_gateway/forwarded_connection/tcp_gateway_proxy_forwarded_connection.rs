use std::sync::Arc;

use encryption::aes::AesKey;
use tokio::sync::Mutex;

use crate::tcp_gateway::{TcpConnectionInner, TcpGatewayContract};

use super::ProxyReceiveBuffer;

#[derive(Clone)]
pub enum TcpGatewayProxyConnectionStatus {
    AwaitingConnection,
    Connected,
    Disconnected(String),
}

impl TcpGatewayProxyConnectionStatus {
    pub fn is_awaiting_connection(&self) -> bool {
        match self {
            TcpGatewayProxyConnectionStatus::AwaitingConnection => true,
            _ => false,
        }
    }

    pub fn is_disconnected(&self) -> bool {
        match self {
            TcpGatewayProxyConnectionStatus::Disconnected(_) => true,
            _ => false,
        }
    }
}

pub struct TcpGatewayProxyForwardedConnectionInner {
    status: TcpGatewayProxyConnectionStatus,
}

impl TcpGatewayProxyForwardedConnectionInner {
    pub fn new() -> Self {
        Self {
            status: TcpGatewayProxyConnectionStatus::AwaitingConnection,
        }
    }
}

pub struct TcpGatewayProxyForwardedConnection {
    inner: Mutex<TcpGatewayProxyForwardedConnectionInner>,
    connection_inner: Arc<TcpConnectionInner>,
    pub connection_id: u32,
    pub gateway_id: Arc<String>,
    pub remote_endpoint: String,
    pub receive_buffer: Mutex<ProxyReceiveBuffer>,
}

impl TcpGatewayProxyForwardedConnection {
    pub fn new(
        connection_id: u32,
        gateway_id: Arc<String>,
        connection_inner: Arc<TcpConnectionInner>,
        remote_endpoint: String,
    ) -> Self {
        Self {
            connection_id,
            connection_inner,
            gateway_id,
            remote_endpoint,
            inner: Mutex::new(TcpGatewayProxyForwardedConnectionInner::new()),
            receive_buffer: Mutex::default(),
        }
    }

    pub fn get_gateway_id(&self) -> &str {
        &self.gateway_id
    }
    pub async fn get_status(&self) -> TcpGatewayProxyConnectionStatus {
        let inner = self.inner.lock().await;
        inner.status.clone()
    }

    pub async fn set_connected(&self) {
        let mut inner = self.inner.lock().await;

        if inner.status.is_awaiting_connection() {
            inner.status = TcpGatewayProxyConnectionStatus::Connected;
        }
    }

    pub async fn disconnect(&self, error: &str, aes_key: &AesKey) -> bool {
        {
            let mut inner = self.inner.lock().await;

            if inner.status.is_disconnected() {
                return false;
            }

            inner.status = TcpGatewayProxyConnectionStatus::Disconnected(error.to_string());
        }

        let connection_error = TcpGatewayContract::ConnectionError {
            connection_id: self.connection_id,
            error,
        };

        self.connection_inner
            .send_payload(&connection_error.to_vec(aes_key))
            .await;

        let mut receive_buffer = self.receive_buffer.lock().await;
        receive_buffer.disconnect(error.to_string());

        true
    }

    pub async fn send_payload(&self, payload: &[u8], aes_key: &AesKey) {
        let send_payload = TcpGatewayContract::ForwardPayload {
            connection_id: self.connection_id,
            compressed: false,
            payload,
        }
        .to_vec(aes_key);

        self.connection_inner.send_payload(&send_payload).await;
    }

    pub async fn receive_payload(&self) -> Result<Vec<u8>, String> {
        let task = {
            let mut buffer_access = self.receive_buffer.lock().await;

            if let Some(payload) = buffer_access.receive_payload()? {
                return Ok(payload);
            }

            buffer_access.engage_awaiter()
        };

        task.get_result().await
    }

    pub async fn enqueue_receive_payload(&self, payload: &[u8]) {
        let mut buffer_access = self.receive_buffer.lock().await;
        buffer_access.set_payload(payload);
    }
}
