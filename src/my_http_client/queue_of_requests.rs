use std::collections::VecDeque;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use rust_extensions::{TaskCompletion, TaskCompletionAwaiter};
use tokio::{io::ReadHalf, sync::Mutex};

use super::MyHttpClientError;

pub type HttpAwaitingTask<TStream> = TaskCompletion<HttpTask<TStream>, MyHttpClientError>;

pub type HttpAwaiterTask<TStream> = TaskCompletionAwaiter<HttpTask<TStream>, MyHttpClientError>;

pub enum HttpTask<TStream: tokio::io::AsyncRead + Send + Sync + 'static> {
    Response(hyper::Response<BoxBody<Bytes, String>>),
    WebsocketUpgrade {
        response: hyper::Response<BoxBody<Bytes, String>>,
        read_part: ReadHalf<TStream>,
    },
}

impl<TStream: tokio::io::AsyncRead + Send + Sync + 'static> HttpTask<TStream> {
    pub fn unwrap_response(self) -> hyper::Response<BoxBody<Bytes, String>> {
        match self {
            HttpTask::Response(response) => response,
            HttpTask::WebsocketUpgrade { response, .. } => response,
        }
    }

    pub fn unwrap_websocket_upgrade(
        self,
    ) -> (hyper::Response<BoxBody<Bytes, String>>, ReadHalf<TStream>) {
        match self {
            HttpTask::WebsocketUpgrade {
                response,
                read_part,
            } => (response, read_part),
            HttpTask::Response(_) => panic!("Can not unwrap as websocket upgrade"),
        }
    }
}

pub struct QueueOfRequests<TStream: tokio::io::AsyncRead + Send + Sync + 'static> {
    queue: Mutex<VecDeque<HttpAwaitingTask<TStream>>>,
}

impl<TStream: tokio::io::AsyncRead + Send + Sync + 'static> QueueOfRequests<TStream> {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn push(&self, task: HttpAwaitingTask<TStream>) {
        self.queue.lock().await.push_back(task);
    }

    pub async fn pop(&self) -> Option<HttpAwaitingTask<TStream>> {
        self.queue.lock().await.pop_front()
    }

    pub async fn notify_connection_lost(&self) {
        let mut queue = self.queue.lock().await;
        while let Some(mut task) = queue.pop_front() {
            task.set_error(MyHttpClientError::Disconnected);
        }
    }
}
