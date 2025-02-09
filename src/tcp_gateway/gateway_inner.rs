use std::sync::{atomic::AtomicBool, Arc};

use tokio::sync::Mutex;

use super::TcpGatewayConnection;

pub struct TcpGatewayInner {
    pub id: Arc<String>,
    pub addr: Arc<String>,
    running: AtomicBool,
    connection: Mutex<Option<Arc<TcpGatewayConnection>>>,
}

impl TcpGatewayInner {
    pub fn new(id: String, addr: String) -> Self {
        Self {
            id: Arc::new(id),
            addr: Arc::new(addr),
            running: AtomicBool::new(true),
            connection: Mutex::default(),
        }
    }

    pub async fn set_gateway_connection(&self, connection: Option<Arc<TcpGatewayConnection>>) {
        let mut connection_access = self.connection.lock().await;
        *connection_access = connection;
    }

    pub async fn get_gateway_connection(&self) -> Option<Arc<TcpGatewayConnection>> {
        let connection_access = self.connection.lock().await;
        connection_access.clone()
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
