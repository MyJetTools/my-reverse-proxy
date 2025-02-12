use std::sync::{atomic::AtomicBool, Mutex};

use futures::task::AtomicWaker;

pub struct ProxyReceiveBuffer {
    pub buffer: Mutex<Option<Vec<u8>>>,
    pub waker: AtomicWaker,
    disconnected: AtomicBool,
}

impl ProxyReceiveBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::new().into()),
            waker: AtomicWaker::new(),
            disconnected: AtomicBool::new(false),
        }
    }

    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn extend_from_slice(&self, payload: &[u8]) {
        let mut buffer_access = self.buffer.lock().unwrap();
        if let Some(buffer) = buffer_access.as_mut() {
            buffer.extend_from_slice(payload);
            self.waker.wake();
        }
    }
    pub fn disconnect(&self) -> bool {
        if self.is_disconnected() {
            return false;
        }
        let mut buffer_access = self.buffer.lock().unwrap();
        self.waker.wake();
        buffer_access.take().is_some()
    }
}
