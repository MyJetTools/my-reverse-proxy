use std::collections::HashMap;

use rust_extensions::{TaskCompletion, TaskCompletionAwaiter};
use tokio::sync::Mutex;

pub enum FileRequestError {
    FileNotFound,
    GatewayDisconnected,
}

#[derive(Default)]
pub struct FileRequestsInner {
    requests: HashMap<u32, TaskCompletion<Vec<u8>, FileRequestError>>,
    next_request_id: u32,
}

impl FileRequestsInner {
    pub fn get_next_request_id(&mut self) -> u32 {
        self.next_request_id += 1;
        self.next_request_id
    }
}

pub struct FileRequests {
    inner: Mutex<FileRequestsInner>,
}

impl FileRequests {
    pub fn new() -> Self {
        Self {
            inner: Mutex::default(),
        }
    }

    pub async fn start_request(&self) -> (TaskCompletionAwaiter<Vec<u8>, FileRequestError>, u32) {
        let mut task_completion = TaskCompletion::new();
        task_completion.set_drop_error(FileRequestError::GatewayDisconnected);
        let awaiter = task_completion.get_awaiter();
        let mut write_access = self.inner.lock().await;
        let request_id = write_access.get_next_request_id();
        write_access.requests.insert(request_id, task_completion);

        (awaiter, request_id)
    }

    pub async fn set_content(&self, request_id: u32, content: Vec<u8>) {
        let item = {
            let mut write_access = self.inner.lock().await;
            write_access.requests.remove(&request_id)
        };

        if let Some(mut item) = item {
            let _ = item.try_set_ok(content);
        }
    }

    pub async fn set_error(&self, request_id: u32, error: FileRequestError) {
        let item = {
            let mut write_access = self.inner.lock().await;
            write_access.requests.remove(&request_id)
        };

        if let Some(mut item) = item {
            let _ = item.try_set_error(error);
        }
    }
}
