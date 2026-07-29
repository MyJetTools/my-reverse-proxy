use std::collections::HashMap;
use std::sync::{Arc, Weak};

use rust_extensions::StrOrString;

use crate::tcp_gateway::TcpConnectionInner;

use super::{ProxyReceiveBuffer, TcpGatewayProxyForwardStream};

/// Registry of active proxy (outbound) connections, keyed by connection id.
/// Lives on `TcpGatewayConnection`; handlers/streams hold a `Weak` to it so a
/// locally-closed stream can drop its own entry without an Arc cycle.
pub type ForwardProxyHandlers =
    tokio::sync::Mutex<HashMap<u32, TcpGatewayProxyForwardConnectionHandler>>;

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
    receive_buffer: Arc<ProxyReceiveBuffer>,
    remote_host: Arc<String>,
    handlers_map: Weak<ForwardProxyHandlers>,
}

impl TcpGatewayProxyForwardConnectionHandler {
    pub fn new(
        connection_id: u32,
        connection_inner: Arc<TcpConnectionInner>,
        remote_host: Arc<String>,
        handlers_map: Weak<ForwardProxyHandlers>,
    ) -> Self {
        Self {
            status: TcpGatewayProxyForwardedConnectionStatus::AcknowledgingConnection,
            connection_id,
            connection_inner,
            receive_buffer: ProxyReceiveBuffer::new().into(),
            remote_host,
            handlers_map,
        }
    }

    pub fn get_remote_host(&self) -> Arc<String> {
        self.remote_host.clone()
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

    pub fn enqueue_receive_payload(&self, payload: &[u8]) {
        self.receive_buffer.extend_from_slice(payload);
    }

    pub fn get_connection(&self) -> TcpGatewayProxyForwardStream {
        TcpGatewayProxyForwardStream {
            receive_buffer: self.receive_buffer.clone(),
            connection_id: self.connection_id,
            gateway_connection_inner: self.connection_inner.clone(),
            handlers_map: self.handlers_map.clone(),
        }
    }

    pub async fn disconnect(&self) {
        self.receive_buffer.disconnect();
    }
}
