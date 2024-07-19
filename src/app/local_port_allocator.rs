use std::sync::atomic::AtomicU16;

pub struct LocalPortAllocator {
    next_port: AtomicU16,
    range_from: u16,
    range_to: u16,
}

impl LocalPortAllocator {
    pub fn new() -> Self {
        Self {
            next_port: AtomicU16::new(0),
            range_from: 0,
            range_to: 65535,
        }
    }

    pub fn next(&self) -> u16 {
        let result = self
            .next_port
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if result == self.range_to {
            self.next_port
                .store(self.range_from, std::sync::atomic::Ordering::Relaxed);
        }

        result
    }
}
