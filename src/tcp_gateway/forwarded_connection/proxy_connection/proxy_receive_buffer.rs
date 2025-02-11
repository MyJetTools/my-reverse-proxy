use std::sync::Mutex;

use futures::task::AtomicWaker;

pub struct ProxyReceiveBuffer {
    pub buffer: Mutex<Option<Vec<u8>>>,
    pub waker: AtomicWaker,
}

impl ProxyReceiveBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::default(),
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
}
