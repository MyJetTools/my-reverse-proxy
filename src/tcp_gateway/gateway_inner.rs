use std::sync::{atomic::AtomicBool, Arc};

pub struct TcpGatewayInner {
    pub id: Arc<String>,
    pub addr: Arc<String>,
    running: AtomicBool,
}

impl TcpGatewayInner {
    pub fn new(id: String, addr: String) -> Self {
        Self {
            id: Arc::new(id),
            addr: Arc::new(addr),
            running: AtomicBool::new(true),
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
