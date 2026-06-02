use std::sync::{Arc, Weak};

use crate::{
    network_stream::*,
    tcp_gateway::{TcpConnectionInner, TcpGatewayContract},
};

use super::{ForwardProxyHandlers, ProxyReceiveBuffer};

#[derive(Clone)]
pub struct TcpGatewayProxyForwardStream {
    pub connection_id: u32,
    pub gateway_connection_inner: Arc<TcpConnectionInner>,
    pub receive_buffer: Arc<ProxyReceiveBuffer>,
    pub handlers_map: Weak<ForwardProxyHandlers>,
}

impl TcpGatewayProxyForwardStream {
    pub fn send_payload(&self, payload: &[u8]) -> bool {
        let frame = TcpGatewayContract::ForwardPayload {
            connection_id: self.connection_id,
            payload: payload.into(),
        }
        .to_plain_frame();

        self.gateway_connection_inner.send_payload(frame)
    }

    pub fn disconnect(&self) {
        // `disconnect()` returns `true` only for whoever flips the buffer to
        // disconnected first. If it returns `false` the connection was already
        // torn down (peer-initiated, via disconnect_forward_proxy_connection),
        // so there is nothing left to clean up or notify.
        if !self.receive_buffer.disconnect() {
            return;
        }

        // We initiated the close locally — notify the peer so it tears down the
        // matching forward connection.
        let frame = TcpGatewayContract::ConnectionError {
            connection_id: self.connection_id,
            error: "",
        }
        .to_plain_frame();

        self.gateway_connection_inner.send_payload(frame);

        // Drop our own handler from the registry so the proxy-connection count
        // goes down. Peer-initiated closes already remove it; this covers the
        // local-close path, which otherwise leaks the entry.
        if let Some(handlers_map) = self.handlers_map.upgrade() {
            let connection_id = self.connection_id;
            crate::app::spawn_named("proxy_forward_stream_cleanup", async move {
                handlers_map.lock().await.remove(&connection_id);
            });
        }
    }
}

#[async_trait::async_trait]
impl NetworkStreamReadPart for TcpGatewayProxyForwardStream {
    async fn read_from_socket(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let result = self.receive_buffer.get_data(buf).await;
        result
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart for TcpGatewayProxyForwardStream {
    async fn shutdown_socket(&mut self) {
        self.disconnect();
    }
    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        if !self.send_payload(buffer) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "No connection",
            ));
        }

        Ok(())
    }

    async fn flush_it(&mut self) -> Result<(), NetworkError> {
        self.gateway_connection_inner.flush().await?;
        Ok(())
    }
}

/*
impl tokio::io::AsyncRead for TcpGatewayProxyForwardStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let mut receive_buffer = self.receive_buffer.buffer.lock().unwrap();
        // Check if there's data available
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

impl tokio::io::AsyncWrite for TcpGatewayProxyForwardStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        if self.receive_buffer.is_disconnected() {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Disconnected",
            )));
        }

        self.send_payload(buf);

        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        self.disconnect();
        std::task::Poll::Ready(Ok(()))
    }
}
 */
