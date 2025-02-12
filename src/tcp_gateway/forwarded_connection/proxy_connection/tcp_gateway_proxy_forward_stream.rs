use std::{sync::Arc, task::Poll};

use crate::tcp_gateway::{TcpConnectionInner, TcpGatewayContract};

use super::ProxyReceiveBuffer;

pub struct TcpGatewayProxyForwardStream {
    pub connection_id: u32,
    pub gateway_connection_inner: Arc<TcpConnectionInner>,
    pub support_compression: bool,
    pub receive_buffer: Arc<ProxyReceiveBuffer>,
}

impl TcpGatewayProxyForwardStream {
    pub fn send_payload(&self, payload: &[u8]) {
        let payload = TcpGatewayContract::ForwardPayload {
            connection_id: self.connection_id,
            payload: payload.into(),
        }
        .to_vec(
            &self.gateway_connection_inner.aes_key,
            self.support_compression,
        );

        let inner = self.gateway_connection_inner.clone();

        tokio::spawn(async move {
            inner.send_payload(payload.as_slice()).await;
        });
    }

    pub fn disconnect(&self) {
        if self.receive_buffer.disconnect() {
            return;
        }
        let payload = TcpGatewayContract::ConnectionError {
            connection_id: self.connection_id,
            error: "disconnect",
        }
        .to_vec(
            &self.gateway_connection_inner.aes_key,
            self.support_compression,
        );

        let inner = self.gateway_connection_inner.clone();

        tokio::spawn(async move {
            inner.send_payload(payload.as_slice()).await;
        });
    }
}
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
