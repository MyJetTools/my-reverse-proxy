use std::sync::Mutex;

use futures::task::AtomicWaker;

pub struct ProxyReceiveBuffer {
    pub buffer: Mutex<Option<Vec<u8>>>,
    pub waker: AtomicWaker,
}

impl ProxyReceiveBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::new().into()),
            waker: AtomicWaker::new(),
        }
    }
    pub fn extend_from_slice(&self, payload: &[u8]) {
        let mut buffer_access = self.buffer.lock().unwrap();
        if let Some(buffer) = buffer_access.as_mut() {
            buffer.extend_from_slice(payload);
            self.waker.wake();
        }
    }
    pub fn disconnect(&self) -> bool {
        let mut buffer_access = self.buffer.lock().unwrap();
        buffer_access.take().is_some()
    }
}
