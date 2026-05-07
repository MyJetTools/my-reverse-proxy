use std::sync::Arc;

use tokio::io::WriteHalf;

pub struct MyHttpClientConnectionContext<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
> {
    pub write_stream: Option<WriteHalf<TStream>>,
    pub queue_to_deliver: Option<Vec<u8>>,
    pub send_to_socket_timeout: std::time::Duration,
}

pub struct WebSocketContextModel {
    pub name: Arc<String>,
}

impl WebSocketContextModel {
    pub fn new(name: Arc<String>) -> Self {
        Self { name }
    }
}
