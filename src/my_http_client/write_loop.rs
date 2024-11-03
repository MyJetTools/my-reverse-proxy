use std::sync::Arc;

use super::MyHttpClientInner;

pub enum WriteLoopEvent {
    Flush(u64),
    Close,
}

pub async fn write_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
>(
    write_part: Arc<MyHttpClientInner<TStream>>,
    mut receiver: tokio::sync::mpsc::Receiver<WriteLoopEvent>,
) {
    while let Some(event) = receiver.recv().await {
        match event {
            WriteLoopEvent::Flush(connection_id) => {
                write_part.flush(connection_id).await;
            }
            WriteLoopEvent::Close => {
                break;
            }
        }
    }
}
