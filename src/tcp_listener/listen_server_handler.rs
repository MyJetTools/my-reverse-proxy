use std::sync::atomic::AtomicBool;

pub struct ListenServerHandler {
    shutting_down: AtomicBool,
    tcp_thread_stopped: AtomicBool,
}

impl ListenServerHandler {
    pub fn new() -> Self {
        Self {
            shutting_down: AtomicBool::new(false),
            tcp_thread_stopped: AtomicBool::new(false),
        }
    }

    pub async fn stop(&self) {
        self.shutting_down
            .store(true, std::sync::atomic::Ordering::Relaxed);

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            if self
                .tcp_thread_stopped
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                break;
            }
        }
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn await_stop(&self) {
        loop {
            if self.is_shutting_down() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    pub fn set_tcp_thread_stopped(&self) {
        self.tcp_thread_stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
