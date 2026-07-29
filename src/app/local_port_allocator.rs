use std::sync::atomic::AtomicU64;

pub struct LocalPortAllocator {
    next_port: AtomicU64,
}

impl LocalPortAllocator {
    pub fn new() -> Self {
        Self {
            next_port: AtomicU64::new(0),
        }
    }

    pub fn next(&self) -> u64 {
        let result = self
            .next_port
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        result
    }
}
