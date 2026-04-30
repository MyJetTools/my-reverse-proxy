use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use rust_extensions::TaskCompletion;

use tokio::{
    io::{AsyncWriteExt, WriteHalf},
    sync::Mutex,
};

use crate::{MyHttpClientDisconnect, MyHttpClientError};

use super::{
    write_loop::WriteLoopEvent, HttpAwaiterTask, HttpAwaitingTask, MyHttpClientConnectionContext,
    MyHttpRequest, QueueOfRequests, WebSocketContextModel,
};

pub enum WritePartState<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
> {
    Connected(MyHttpClientConnectionContext<TStream>),
    UpgradedToWebSocket(WebSocketContextModel),
    Disconnected,
    Disposed,
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    WritePartState<TStream>
{
    pub fn get_payload_to_send(
        &mut self,
    ) -> Option<(&mut WriteHalf<TStream>, Vec<u8>, Duration)> {
        match self {
            WritePartState::Connected(inner) => {
                let payload = inner.queue_to_deliver.take()?;
                let write_stream = inner.write_stream.as_mut().unwrap();
                Some((write_stream, payload, inner.send_to_socket_timeout))
            }
            WritePartState::UpgradedToWebSocket(_) => None,
            WritePartState::Disconnected => None,
            WritePartState::Disposed => None,
        }
    }
    pub fn is_disposed(&self) -> bool {
        matches!(self, WritePartState::Disposed)
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
            WritePartState::UpgradedToWebSocket(_) => Err(MyHttpClientError::UpgradedToWebSocket),

            WritePartState::Disconnected => Err(MyHttpClientError::Disconnected),
            WritePartState::Disposed => Err(MyHttpClientError::Disposed),
        }
    }
}

pub struct MyHttpClientInner<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
> {
    pub state: Mutex<(
        WritePartState<TStream>,
        Option<tokio::sync::mpsc::Sender<WriteLoopEvent>>,
    )>,

    connection_id: AtomicU64,
    waiting_ws_upgrade: AtomicBool,
    pub queue_of_requests: QueueOfRequests<TStream>,

    pub metrics: Option<Arc<dyn super::MyHttpClientMetrics + Send + Sync + 'static>>,
    pub name: Arc<String>,
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    MyHttpClientInner<TStream>
{
    pub fn new(
        name: String,
        metrics: Option<Arc<dyn super::MyHttpClientMetrics + Send + Sync + 'static>>,
    ) -> Self {
        let result = Self {
            state: Mutex::new((WritePartState::Disconnected, None)),
            connection_id: AtomicU64::new(0),
            waiting_ws_upgrade: AtomicBool::new(false),
            queue_of_requests: QueueOfRequests::new(),

            metrics,
            name: Arc::new(name),
        };

        if let Some(metrics) = &result.metrics {
            metrics.instance_created(&result.name);
        }
        result
    }

    pub async fn set_sender(&self, sender: tokio::sync::mpsc::Sender<WriteLoopEvent>) {
        let mut state = self.state.lock().await;
        state.1 = Some(sender);
    }

    pub async fn new_connection(
        &self,
        connection_id: u64,
        write_stream: WriteHalf<TStream>,
        send_to_socket_timeout: std::time::Duration,
    ) {
        let mut state = self.state.lock().await;

        if state.0.is_disposed() {
            panic!("Disposed");
        }

        self.process_disconnect(&mut state.0, WritePartState::Disconnected)
            .await;

        state.0 = WritePartState::Connected(MyHttpClientConnectionContext {
            write_stream: Some(write_stream),
            queue_to_deliver: None,
            send_to_socket_timeout,
        });

        self.waiting_ws_upgrade.store(false, Ordering::Relaxed);
        self.connection_id.store(connection_id, Ordering::Release);

        if let Some(metrics) = self.metrics.as_ref() {
            metrics.tcp_connect(&self.name);
        }
    }

    pub fn is_my_connection_id(&self, connection_id: u64) -> bool {
        self.connection_id.load(Ordering::Acquire) == connection_id
    }

    pub async fn send(
        &self,
        req: &MyHttpRequest,
    ) -> Result<(HttpAwaiterTask<TStream>, u64), MyHttpClientError> {
        let mut writer = self.state.lock().await;

        let (awaiter, connection_id) = {
            let connection_context = writer.0.unwrap_as_connected_mut()?;
            let mut task = TaskCompletion::new();
            let awaiter = task.get_awaiter();

            self.queue_of_requests.push(task);

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

            (awaiter, self.connection_id.load(Ordering::Relaxed))
        };

        let _ = writer
            .1
            .as_ref()
            .unwrap()
            .send(WriteLoopEvent::Flush(connection_id))
            .await;

        Ok((awaiter, connection_id))
    }

    pub async fn upgrade_to_websocket(
        &self,
        connection_id: u64,
    ) -> Result<WriteHalf<TStream>, MyHttpClientError> {
        let mut state = self.state.lock().await;

        match &mut state.0 {
            WritePartState::Connected(context) => {
                if self.connection_id.load(Ordering::Relaxed) != connection_id {
                    return Err(MyHttpClientError::Disconnected);
                }

                let result = context.write_stream.take();

                state.0 =
                    WritePartState::UpgradedToWebSocket(WebSocketContextModel::new(self.name.clone()));

                if let Some(metrics) = self.metrics.as_ref() {
                    metrics.upgraded_to_websocket(&self.name);
                }
                Ok(result.unwrap())
            }
            WritePartState::UpgradedToWebSocket(_) => Err(MyHttpClientError::UpgradedToWebSocket),
            WritePartState::Disconnected => Err(MyHttpClientError::Disconnected),
            WritePartState::Disposed => Err(MyHttpClientError::Disposed),
        }
    }

    pub fn pop_request(
        &self,
        connection_id: u64,
        web_socket_upgrade: bool,
    ) -> Option<HttpAwaitingTask<TStream>> {
        if self.connection_id.load(Ordering::Acquire) != connection_id {
            return None;
        }

        if web_socket_upgrade {
            self.waiting_ws_upgrade.store(true, Ordering::Relaxed);
        }

        self.queue_of_requests.pop()
    }

    pub async fn flush(&self, connection_id: u64) {
        let mut state = self.state.lock().await;

        if self.connection_id.load(Ordering::Relaxed) != connection_id {
            return;
        }

        let mut has_error = false;
        if let Some((stream, payload, send_to_socket_timeout)) = state.0.get_payload_to_send() {
            for chunk in payload.chunks(1024 * 1024) {
                let future = stream.write_all(chunk);

                let result = tokio::time::timeout(send_to_socket_timeout, future).await;

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
            self.connection_id.store(0, Ordering::Release);
            self.process_disconnect(&mut state.0, WritePartState::Disconnected)
                .await;
        }
    }

    pub async fn disconnect(&self, connection_id: u64) {
        let mut state = self.state.lock().await;

        if self.connection_id.load(Ordering::Relaxed) != connection_id {
            return;
        }

        if matches!(
            state.0,
            WritePartState::Disconnected | WritePartState::Disposed
        ) {
            return;
        }

        self.connection_id.store(0, Ordering::Release);
        self.process_disconnect(&mut state.0, WritePartState::Disconnected)
            .await;
    }

    async fn process_disconnect(
        &self,
        state: &mut WritePartState<TStream>,
        new_status: WritePartState<TStream>,
    ) {
        match &mut *state {
            WritePartState::Connected(context) => {
                if let Some(metrics) = self.metrics.as_ref() {
                    metrics.tcp_disconnect(&self.name);
                }
                if let Some(mut write_stream) = context.write_stream.take() {
                    let _ = write_stream.shutdown().await;
                }
                self.queue_of_requests.notify_connection_lost();
            }
            WritePartState::UpgradedToWebSocket(_) => {
                if let Some(metrics) = self.metrics.as_ref() {
                    metrics.tcp_disconnect(&self.name);
                }
            }
            _ => {}
        }

        *state = new_status;
    }

    pub async fn read_loop_stopped(&self, connection_id: u64) {
        if self.waiting_ws_upgrade.load(Ordering::Relaxed) {
            return;
        }

        let mut state = self.state.lock().await;

        if self.connection_id.load(Ordering::Relaxed) != connection_id {
            return;
        }

        if !matches!(state.0, WritePartState::Connected(_)) {
            return;
        }

        self.connection_id.store(0, Ordering::Release);
        self.process_disconnect(&mut state.0, WritePartState::Disconnected)
            .await;
    }

    pub async fn dispose(&self) {
        let mut state = self.state.lock().await;
        self.connection_id.store(0, Ordering::Release);
        self.process_disconnect(&mut state.0, WritePartState::Disposed)
            .await;

        if let Some(sender) = state.1.as_ref() {
            let _ = sender.send(WriteLoopEvent::Close).await;
        }
    }
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static> Drop
    for MyHttpClientInner<TStream>
{
    fn drop(&mut self) {
        if let Some(metrics) = self.metrics.as_ref() {
            metrics.instance_disposed(&self.name);
        }
    }
}

pub struct MyHttpClientDisconnection<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
> {
    inner: Arc<MyHttpClientInner<TStream>>,
    connection_id: u64,
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    MyHttpClientDisconnection<TStream>
{
    pub fn new(inner: Arc<MyHttpClientInner<TStream>>, connection_id: u64) -> Self {
        Self {
            inner,
            connection_id,
        }
    }
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    MyHttpClientDisconnect for MyHttpClientDisconnection<TStream>
{
    fn disconnect(&self) {
        let inner = self.inner.clone();
        let connection_id = self.connection_id;

        crate::spawn_named("myhttp_h1_disconnect", async move {
            inner.disconnect(connection_id).await;
        });
    }

    fn web_socket_disconnect(&self) {
        if let Some(metrics) = self.inner.metrics.as_ref() {
            metrics.websocket_is_disconnected(&self.inner.name);
        }

        let inner = self.inner.clone();
        let connection_id = self.connection_id;

        crate::spawn_named("myhttp_h1_websocket_disconnect", async move {
            inner.disconnect(connection_id).await;
        });
    }

    fn get_connection_id(&self) -> u64 {
        self.connection_id
    }
}
