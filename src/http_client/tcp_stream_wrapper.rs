use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::{AsyncRead, AsyncWrite};
use tokio_util::compat::TokioAsyncReadCompatExt;

pub struct TcpStreamWrapper(tokio::net::TcpStream);

impl TcpStreamWrapper {
    pub fn new(tcp_stream: tokio::net::TcpStream) -> Self {
        Self(tcp_stream)
    }
}

impl AsyncRead for TcpStreamWrapper {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let result = self.0.compat();
    }
}

impl AsyncWrite for TcpStreamWrapper {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        todo!("Implement")
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        todo!("Implement")
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        todo!("Implement")
    }
}
