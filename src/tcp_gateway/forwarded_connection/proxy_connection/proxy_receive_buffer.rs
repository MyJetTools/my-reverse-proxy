use std::sync::Mutex;

use futures::task::AtomicWaker;

pub struct ProxyReceiveBuffer {
    pub buffer: Mutex<Vec<u8>>,
    pub waker: AtomicWaker,
}

impl ProxyReceiveBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::new()),
            waker: AtomicWaker::new(),
        }
    }
    pub fn extend_from_slice(&self, payload: &[u8]) {
        let mut buffer_access = self.buffer.lock().unwrap();
        buffer_access.extend_from_slice(payload);
        self.waker.wake();
    }
}
