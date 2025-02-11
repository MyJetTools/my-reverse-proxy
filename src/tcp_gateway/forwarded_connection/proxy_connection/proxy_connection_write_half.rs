use std::sync::Arc;

use crate::tcp_gateway::{TcpConnectionInner, TcpGatewayContract};

use super::ProxyReceiveBuffer;

pub struct ProxyConnectionWriteHalf {
    pub gateway_id: Arc<String>,
    pub remote_host: Arc<String>,
    pub connection_id: u32,
    pub gateway_connection_inner: Arc<TcpConnectionInner>,
    pub support_compression: bool,
    pub receive_buffer: Arc<ProxyReceiveBuffer>,
}

impl ProxyConnectionWriteHalf {
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

impl tokio::io::AsyncWrite for ProxyConnectionWriteHalf {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        println!(
            "Sending payload {} to {}",
            buf.len(),
            self.remote_host.as_str()
        );
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
