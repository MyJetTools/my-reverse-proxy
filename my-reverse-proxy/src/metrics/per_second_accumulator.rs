use std::sync::atomic::AtomicUsize;

pub struct PerSecondAccumulator {
    value: AtomicUsize,
}

impl PerSecondAccumulator {
    pub fn new() -> Self {
        Self {
            value: AtomicUsize::new(0),
        }
    }

    pub fn add(&self, value: usize) {
        self.value
            .fetch_add(value, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_per_second(&self) -> usize {
        self.value.swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}
