use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
};

use encryption::aes::AesKey;
use tokio::sync::Mutex;

use super::TcpGatewayConnection;

pub struct TcpGatewayInner {
    pub gateway_id: Arc<String>,
    pub gateway_host: Arc<String>,
    running: AtomicBool,
    connection: Mutex<HashMap<String, Arc<TcpGatewayConnection>>>,
    pub encryption: Arc<AesKey>,
    pub allow_incoming_forward_connections: bool,
}

impl TcpGatewayInner {
    pub fn new(
        gateway_id: String,
        addr: String,
        allow_incoming_forward_connections: bool,
        encryption: AesKey,
    ) -> Self {
        Self {
            gateway_id: Arc::new(gateway_id),
            gateway_host: Arc::new(addr),
            running: AtomicBool::new(true),
            connection: Mutex::default(),
            encryption: Arc::new(encryption),
            allow_incoming_forward_connections,
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

    pub async fn get_gateway_connections(&self) -> Vec<Arc<TcpGatewayConnection>> {
        let connection_access = self.connection.lock().await;
        connection_access.values().cloned().collect()
    }

    pub fn get_gateway_id(&self) -> &str {
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
