use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
};

use tokio::sync::Mutex;

use super::TcpGatewayConnection;

pub struct TcpGatewayInner {
    pub gateway_id: Arc<String>,
    pub addr: Arc<String>,
    running: AtomicBool,
    connection: Mutex<HashMap<String, Arc<TcpGatewayConnection>>>,
}

impl TcpGatewayInner {
    pub fn new(gateway_id: String, addr: String) -> Self {
        Self {
            gateway_id: Arc::new(gateway_id),
            addr: Arc::new(addr),
            running: AtomicBool::new(true),
            connection: Mutex::default(),
        }
    }

    pub async fn set_gateway_connection(
        &self,
        id: &str,
        connection: Option<Arc<TcpGatewayConnection>>,
    ) {
        let mut connection_access = self.connection.lock().await;

        match connection {
            Some(connection) => {
                connection_access.insert(id.to_string(), connection);
            }
            None => {
                connection_access.remove(id);
            }
        }
    }

    pub async fn get_gateway_connection(&self, id: &str) -> Option<Arc<TcpGatewayConnection>> {
        let connection_access = self.connection.lock().await;
        connection_access.get(id).cloned()
    }

    pub fn get_id(&self) -> &str {
        &self.gateway_id
    }

    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::Relaxed)
    }
}
