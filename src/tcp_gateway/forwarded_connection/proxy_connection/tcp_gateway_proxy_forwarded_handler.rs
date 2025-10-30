use std::sync::Arc;

use rust_extensions::StrOrString;

use crate::tcp_gateway::TcpConnectionInner;

use super::{ProxyReceiveBuffer, TcpGatewayProxyForwardStream};

#[derive(Clone)]
pub enum TcpGatewayProxyForwardedConnectionStatus {
    AcknowledgingConnection,
    Connected,
    Disconnected(StrOrString<'static>),
}

impl TcpGatewayProxyForwardedConnectionStatus {
    pub fn is_awaiting_connection(&self) -> bool {
        match self {
            Self::AcknowledgingConnection => true,
            _ => false,
        }
    }
}

pub struct TcpGatewayProxyForwardConnectionHandler {
    pub status: TcpGatewayProxyForwardedConnectionStatus,
    connection_inner: Arc<TcpConnectionInner>,
    pub connection_id: u32,
    //pub receive_buffer: Mutex<ProxyReceiveBuffer>,
    pub support_compression: bool,
    receive_buffer: Arc<ProxyReceiveBuffer>,
}

impl TcpGatewayProxyForwardConnectionHandler {
    pub fn new(
        connection_id: u32,
        connection_inner: Arc<TcpConnectionInner>,
        support_compression: bool,
    ) -> Self {
        Self {
            status: TcpGatewayProxyForwardedConnectionStatus::AcknowledgingConnection,
            connection_id,
            connection_inner,
            receive_buffer: ProxyReceiveBuffer::new().into(),
            support_compression,
        }
    }

    pub fn get_status(&self) -> TcpGatewayProxyForwardedConnectionStatus {
        self.status.clone()
    }

    pub fn set_connected(&mut self) {
        if self.status.is_awaiting_connection() {
            self.status = TcpGatewayProxyForwardedConnectionStatus::Connected;
        }
    }

    pub fn set_connection_error(&mut self, error: String) {
        match self.status {
            TcpGatewayProxyForwardedConnectionStatus::AcknowledgingConnection => {
                self.status = TcpGatewayProxyForwardedConnectionStatus::Disconnected(error.into());
            }
            TcpGatewayProxyForwardedConnectionStatus::Connected => {
                self.status = TcpGatewayProxyForwardedConnectionStatus::Disconnected(error.into());
            }
            TcpGatewayProxyForwardedConnectionStatus::Disconnected(_) => {}
        }
    }

    pub async fn enqueue_receive_payload(&self, payload: &[u8]) {
        self.receive_buffer.extend_from_slice(payload).await;
    }

    pub fn get_connection(&self) -> TcpGatewayProxyForwardStream {
        TcpGatewayProxyForwardStream {
            receive_buffer: self.receive_buffer.clone(),
            connection_id: self.connection_id,
            gateway_connection_inner: self.connection_inner.clone(),
            support_compression: self.support_compression,
        }
    }

    pub async fn disconnect(&self) {
        self.receive_buffer.disconnect().await;
    }
}
