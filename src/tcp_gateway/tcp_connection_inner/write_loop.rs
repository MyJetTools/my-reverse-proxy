use std::sync::Arc;

use super::TcpConnectionInner;

pub fn start_write_loop(inner: Arc<TcpConnectionInner>, receiver: tokio::sync::mpsc::Receiver<()>) {
    tokio::spawn(write_loop(inner, receiver));
}

async fn write_loop(inner: Arc<TcpConnectionInner>, mut receiver: tokio::sync::mpsc::Receiver<()>) {
    loop {
        if receiver.recv().await.is_none() {
            inner.disconnect().await;
            break;
        }
        if !inner.push_payload_to_socket().await {
            inner.disconnect().await;
            break;
        }
    }
}
