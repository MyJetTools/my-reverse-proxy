use rust_extensions::*;

pub enum ProxyReceiveBuffer {
    Buffer(Option<Vec<u8>>),
    Awaiting(TaskCompletion<Vec<u8>, String>),
    Disconnected(String),
}

impl ProxyReceiveBuffer {
    pub fn receive_payload(&mut self) -> Result<Option<Vec<u8>>, String> {
        match self {
            ProxyReceiveBuffer::Buffer(buffer) => return Ok(buffer.take()),
            ProxyReceiveBuffer::Awaiting(_) => Ok(None),
            ProxyReceiveBuffer::Disconnected(err) => Err(err.to_string()),
        }
    }

    pub fn engage_awaiter(&mut self) -> TaskCompletionAwaiter<Vec<u8>, String> {
        let mut task_completion = TaskCompletion::new();
        let awaiter = task_completion.get_awaiter();
        *self = ProxyReceiveBuffer::Awaiting(task_completion);
        awaiter
    }

    pub fn set_payload(&mut self, payload: &[u8]) {
        match self {
            ProxyReceiveBuffer::Buffer(buffer) => match buffer {
                Some(buffer) => buffer.extend_from_slice(payload),
                None => *buffer = Some(payload.to_vec()),
            },
            ProxyReceiveBuffer::Awaiting(task_completion) => {
                task_completion.set_ok(payload.to_vec());
                *self = ProxyReceiveBuffer::Buffer(None)
            }
            ProxyReceiveBuffer::Disconnected(_) => {}
        }
    }

    pub fn disconnect(&mut self, message: String) {
        *self = ProxyReceiveBuffer::Disconnected(message);
    }
}

impl Default for ProxyReceiveBuffer {
    fn default() -> Self {
        Self::Buffer(None)
    }
}
