use rust_extensions::TaskCompletion;
use tokio::sync::Mutex;

#[derive(Default)]
pub struct AwaitingContent {
    buffer: Vec<u8>,
    awaiter: Option<TaskCompletion<(), std::io::Error>>,
    disconnected: bool,
}

pub struct ProxyReceiveBuffer {
    pub buffer: Mutex<AwaitingContent>,
}

impl ProxyReceiveBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(AwaitingContent::default()),
        }
    }

    pub async fn get_data(&self, out: &mut [u8]) -> Result<usize, std::io::Error> {
        loop {
            let awaiter = {
                let mut write_access = self.buffer.lock().await;
                if write_access.disconnected {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        "Can not read. Disconnected",
                    ));
                }
                if write_access.buffer.len() > 0 {
                    let size = if write_access.buffer.len() > out.len() {
                        out.copy_from_slice(write_access.buffer.drain(..out.len()).as_slice());
                        out.len()
                    } else {
                        let result = write_access.buffer.len();
                        out[..write_access.buffer.len()].copy_from_slice(&write_access.buffer);
                        write_access.buffer.clear();
                        result
                    };

                    return Ok(size);
                }

                let mut task_completion = TaskCompletion::new();

                let awaiter = task_completion.get_awaiter();

                write_access.awaiter = Some(task_completion);
                awaiter
            };

            let awaiter = awaiter.get_result().await;
            awaiter?;
        }
    }

    pub async fn extend_from_slice(&self, payload: &[u8]) {
        let mut buffer_access = self.buffer.lock().await;
        buffer_access.buffer.extend_from_slice(payload);
        if let Some(mut task_completion) = buffer_access.awaiter.take() {
            task_completion.set_ok(());
        }
    }
    pub async fn disconnect(&self) -> bool {
        let mut buffer_access = self.buffer.lock().await;
        if buffer_access.disconnected {
            return false;
        }

        buffer_access.disconnected = true;
        if let Some(mut task_completion) = buffer_access.awaiter.take() {
            task_completion.set_error(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "Can not disconnect already disconnected",
            ));
        }

        true
    }
}
