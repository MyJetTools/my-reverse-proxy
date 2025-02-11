use std::{sync::Arc, task::Poll};

use super::ProxyReceiveBuffer;

pub struct ProxyConnectionReadHalf {
    pub(crate) receive_buffer: Arc<ProxyReceiveBuffer>,
    pub gateway_id: Arc<String>,
    pub remote_endpoint: Arc<String>,
    pub connection_id: u32,
}

impl tokio::io::AsyncRead for ProxyConnectionReadHalf {
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
