use rust_extensions::{TaskCompletion, UnsafeValue};

use tokio::{
    io::{AsyncWriteExt, WriteHalf},
    sync::Mutex,
};

use crate::http_client::HTTP_CLIENT_TIMEOUT;

use super::{
    write_loop::WriteLoopEvent, HttpAwaiterTask, HttpAwaitingTask, MyHttpClientConnectionContext,
    MyHttpClientError, MyHttpRequest, QueueOfRequests,
};

pub enum WritePartState<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
> {
    Connected(MyHttpClientConnectionContext<TStream>),
    UpgradedToWebSocket,
    Disconnected,
    Disposed,
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    WritePartState<TStream>
{
    pub fn get_payload_to_send(&mut self) -> Option<(&mut WriteHalf<TStream>, Vec<u8>, u64)> {
        match self {
            WritePartState::Connected(inner) => {
                let payload = inner.queue_to_deliver.take();

                if payload.is_none() {
                    return None;
                }

                let write_stream = inner.write_stream.as_mut().unwrap();

                Some((write_stream, payload.unwrap(), inner.connection_id))
            }
            WritePartState::UpgradedToWebSocket => None,
            WritePartState::Disconnected => None,
            WritePartState::Disposed => None,
        }
    }

    pub fn disposed(&self) -> bool {
        match self {
            WritePartState::Disposed => true,
            _ => false,
        }
    }
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    WritePartState<TStream>
{
    pub fn unwrap_as_connected_mut(
        &mut self,
    ) -> Result<&mut MyHttpClientConnectionContext<TStream>, MyHttpClientError> {
        match self {
            WritePartState::Connected(inner) => Ok(inner),
            WritePartState::UpgradedToWebSocket => Err(MyHttpClientError::UpgradedToWebSocket),
            WritePartState::Disconnected => Err(MyHttpClientError::Disconnected),
            WritePartState::Disposed => Err(MyHttpClientError::Disposed),
        }
    }

    pub fn is_active_connection(&self, connection_id: u64) -> bool {
        match self {
            WritePartState::Connected(inner) => inner.connection_id == connection_id,
            _ => false,
        }
    }
}

pub struct MyHttpClientInner<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
> {
    connected: UnsafeValue<bool>,
    state: Mutex<WritePartState<TStream>>,
    write_signal: tokio::sync::mpsc::Sender<WriteLoopEvent>,
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    MyHttpClientInner<TStream>
{
    pub fn new(write_signal: tokio::sync::mpsc::Sender<WriteLoopEvent>) -> Self {
        Self {
            state: Mutex::new(WritePartState::Disconnected),
            connected: UnsafeValue::new(true),
            write_signal,
        }
    }

    pub async fn new_connection(&self, connection_id: u64, write_stream: WriteHalf<TStream>) {
        let mut state = self.state.lock().await;

        *state = WritePartState::Connected(MyHttpClientConnectionContext {
            write_stream: Some(write_stream),
            queue_to_deliver: None,
            connection_id,
            queue_of_requests: QueueOfRequests::new(),
        });
    }

    pub async fn is_my_connection_id(&self, connection_id: u64) -> bool {
        let state = self.state.lock().await;
        match &*state {
            WritePartState::Connected(context) => context.connection_id == connection_id,
            _ => false,
        }
    }

    pub async fn send(
        &self,
        req: &MyHttpRequest,
    ) -> Result<(HttpAwaiterTask<TStream>, u64), MyHttpClientError> {
        let mut writer = self.state.lock().await;

        let connection_context = writer.unwrap_as_connected_mut()?;
        let mut task = TaskCompletion::new();
        let awaiter = task.get_awaiter();
        connection_context.queue_of_requests.push(task).await;

        match connection_context.queue_to_deliver.as_mut() {
            Some(vec) => {
                req.write_to(vec);
            }
            None => {
                let mut vec = Vec::new();
                req.write_to(&mut vec);
                connection_context.queue_to_deliver = Some(vec);
            }
        }

        let _ = self
            .write_signal
            .send(WriteLoopEvent::Flush(connection_context.connection_id))
            .await;

        Ok((awaiter, connection_context.connection_id))
    }

    pub async fn upgrade_to_websocket(
        &self,
        connection_id: u64,
    ) -> Result<WriteHalf<TStream>, MyHttpClientError> {
        let mut state = self.state.lock().await;

        match &mut *state {
            WritePartState::Connected(context) => {
                if context.connection_id != connection_id {
                    return Err(MyHttpClientError::Disconnected);
                }

                let result = context.write_stream.take();
                *state = WritePartState::UpgradedToWebSocket;

                Ok(result.unwrap())
            }
            WritePartState::UpgradedToWebSocket => {
                return Err(MyHttpClientError::UpgradedToWebSocket);
            }
            WritePartState::Disconnected => {
                return Err(MyHttpClientError::Disconnected);
            }
            WritePartState::Disposed => {
                return Err(MyHttpClientError::Disposed);
            }
        }
    }

    pub async fn pop_request(&self, connection_id: u64) -> Option<HttpAwaitingTask<TStream>> {
        let mut state = self.state.lock().await;
        match &mut *state {
            WritePartState::Connected(context) => {
                if context.connection_id != connection_id {
                    return None;
                }

                context.queue_of_requests.pop().await
            }
            _ => None,
        }
    }

    pub async fn flush(&self, connection_id: u64) {
        let mut state = self.state.lock().await;

        let mut has_error = false;
        if let Some((stream, payload, payload_connection_id)) = state.get_payload_to_send() {
            if payload_connection_id != connection_id {
                return;
            }

            for chunk in payload.chunks(1024 * 1024) {
                let future = stream.write_all(chunk);

                let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

                if result.is_err() {
                    has_error = true;
                    break;
                }

                let result = result.unwrap();

                if result.is_err() {
                    has_error = true;
                    break;
                }
            }
        }

        if has_error {
            *state = WritePartState::Disconnected;
        }
    }

    pub async fn disconnect(&self, connection_id: u64) {
        if !self.connected.get_value() {
            return;
        }

        let mut state = self.state.lock().await;

        if !state.is_active_connection(connection_id) {
            return;
        }

        match &mut *state {
            WritePartState::Connected(context) => {
                context.queue_of_requests.notify_connection_lost().await;
            }
            _ => {}
        }

        *state = WritePartState::Disconnected;

        self.connected.set_value(false);
    }

    pub async fn dispose(&self) {
        let mut state = self.state.lock().await;
        if state.disposed() {
            return;
        }

        *state = WritePartState::Disposed;
        let _ = self.write_signal.send(WriteLoopEvent::Close).await;
    }
}
