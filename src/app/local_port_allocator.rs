use std::sync::atomic::AtomicU16;

pub struct LocalPortAllocator {
    next_port: AtomicU16,
}

impl LocalPortAllocator {
    pub fn new() -> Self {
        Self {
            next_port: AtomicU16::new(65535),
        }
    }

    pub fn next(&self) -> u16 {
        let result = self
            .next_port
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        if result == 10000 {
            self.next_port
                .store(65535, std::sync::atomic::Ordering::Relaxed);
        }

        result
    }
}
