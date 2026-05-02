use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::tcp_utils::WsTrafficRecorder;

/// Wraps an `AsyncRead + AsyncWrite` stream and records WS traffic on every
/// successful read. Writes pass through unchanged.
pub struct MeteredStream<S> {
    inner: S,
    recorder: WsTrafficRecorder,
}

impl<S> MeteredStream<S> {
    pub fn new(inner: S, recorder: WsTrafficRecorder) -> Self {
        Self { inner, recorder }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for MeteredStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &result {
            let n = buf.filled().len().saturating_sub(before);
            if n > 0 {
                self.recorder.record(n as u64);
            }
        }
        result
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for MeteredStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
