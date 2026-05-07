use std::collections::VecDeque;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use parking_lot::Mutex;
use rust_extensions::{TaskCompletion, TaskCompletionAwaiter};
use tokio::io::ReadHalf;

use crate::MyHttpClientError;

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

impl<TStream: tokio::io::AsyncRead + Send + Sync + 'static> Default for QueueOfRequests<TStream> {
    fn default() -> Self {
        Self::new()
    }
}

impl<TStream: tokio::io::AsyncRead + Send + Sync + 'static> QueueOfRequests<TStream> {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push(&self, task: HttpAwaitingTask<TStream>) {
        self.queue.lock().push_back(task);
    }

    pub fn pop(&self) -> Option<HttpAwaitingTask<TStream>> {
        self.queue.lock().pop_front()
    }

    pub fn notify_connection_lost(&self) {
        let mut queue = self.queue.lock();
        while let Some(mut task) = queue.pop_front() {
            let _ = task.try_set_error(MyHttpClientError::Disconnected);
        }
    }
}
