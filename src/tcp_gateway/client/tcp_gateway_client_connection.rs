use std::{collections::HashMap, sync::Arc};

use rust_extensions::date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds};
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use crate::tcp_gateway::{TcpConnectionInner, TcpGatewayConnection, TcpGatewayContract};

use super::TcpGatewayClientForwardConnection;

pub struct TcpGatewayClientConnection {
    pub gateway_id: Arc<String>,
    pub addr: Arc<String>,
    inner: Arc<TcpConnectionInner>,
    last_incoming_payload_time: AtomicDateTimeAsMicroseconds,
    forward_connections: Mutex<HashMap<u32, Arc<TcpGatewayClientForwardConnection>>>,
}

impl TcpGatewayClientConnection {
    pub fn new(gateway_id: Arc<String>, addr: Arc<String>, write_half: OwnedWriteHalf) -> Self {
        let (inner, receiver) = TcpConnectionInner::new(write_half);
        let inner = Arc::new(inner);
        let result = Self {
            gateway_id,
            addr,
            inner: inner.clone(),
            forward_connections: Mutex::default(),
            last_incoming_payload_time: AtomicDateTimeAsMicroseconds::now(),
        };

        super::super::tcp_connection_inner::start_write_loop(inner, receiver);

        result
    }
}

#[async_trait::async_trait]
impl TcpGatewayConnection for TcpGatewayClientConnection {
    type ForwardConnection = TcpGatewayClientForwardConnection;

    fn get_addr(&self) -> &str {
        self.addr.as_str()
    }
    async fn disconnect(&self) {
        self.inner.disconnect().await;
    }
    async fn send_payload(&self, payload: &TcpGatewayContract) -> bool {
        let vec = payload.to_vec();
        self.inner.send_payload(vec.as_slice()).await
    }

    fn set_last_incoming_payload_time(&self, time: DateTimeAsMicroseconds) {
        self.last_incoming_payload_time.update(time);
    }

    fn get_last_incoming_payload_time(&self) -> DateTimeAsMicroseconds {
        self.last_incoming_payload_time.as_date_time()
    }

    async fn add_forward_connection(
        &self,
        connection_id: u32,
        connection: Arc<Self::ForwardConnection>,
    ) {
        let mut write_access = self.forward_connections.lock().await;
        write_access.insert(connection_id, connection);
    }

    async fn get_forward_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<Self::ForwardConnection>> {
        let write_access = self.forward_connections.lock().await;
        write_access.get(&connection_id).cloned()
    }

    async fn has_forward_connection(&self, connection_id: u32) -> bool {
        let read_access = self.forward_connections.lock().await;
        read_access.contains_key(&connection_id)
    }

    async fn remove_forward_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<Self::ForwardConnection>> {
        let mut write_access = self.forward_connections.lock().await;
        write_access.remove(&connection_id)
    }
}
