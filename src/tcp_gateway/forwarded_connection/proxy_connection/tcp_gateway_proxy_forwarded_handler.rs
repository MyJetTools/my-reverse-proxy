use std::sync::Arc;

use rust_extensions::StrOrString;

use crate::tcp_gateway::TcpConnectionInner;

use super::{ProxyReceiveBuffer, TcpGatewayProxyForwardStream};

#[derive(Clone)]
pub enum TcpGatewayProxyForwardedConnectionStatus {
    AwaitingConnection,
    Connected,
    Disconnected(StrOrString<'static>),
}

impl TcpGatewayProxyForwardedConnectionStatus {
    pub fn is_awaiting_connection(&self) -> bool {
        match self {
            Self::AwaitingConnection => true,
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
            status: TcpGatewayProxyForwardedConnectionStatus::AwaitingConnection,
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
        if self.status.is_awaiting_connection() {
            self.status = TcpGatewayProxyForwardedConnectionStatus::Disconnected(error.into());
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
            support_compression: self.support_compression,
        }
    }
}

/*
impl tokio::io::AsyncRead for TcpGatewayProxyForwardedConnection {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Check if there's data available
        let mut receive_buffer = self.receive_buffer.buffer.lock().unwrap();
        if let Some(receive_buffer) = receive_buffer.as_mut() {
            if receive_buffer.len() == 0 {
                // No data, register waker and return Pending

                self.receive_buffer.waker.register(cx.waker());

                return Poll::Pending;
            }

            // Read available data into the buffer

            if buf.remaining() >= receive_buffer.len() {
                buf.put_slice(receive_buffer.drain(..).as_slice());
            } else {
                buf.put_slice(receive_buffer.drain(..buf.remaining()).as_slice());
            }
            return Poll::Ready(Ok(()));
        }

        return Poll::Ready(Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "Disconnected",
        )));
    }
}

 */
