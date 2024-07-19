use std::sync::atomic::AtomicU16;

pub struct LocalPortAllocator {
    next_port: AtomicU16,
    range_from: u16,
    range_to: u16,
}

impl LocalPortAllocator {
    pub fn new(range_from: u16, range_to: u16) -> Self {
        if range_from < 1024 {
            panic!(
                "range_from must be greater than 1024. Value is {}",
                range_from
            );
        }
        Self {
            next_port: AtomicU16::new(range_from),
            range_from,
            range_to,
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
