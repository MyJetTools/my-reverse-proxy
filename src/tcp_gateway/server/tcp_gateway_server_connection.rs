use std::{collections::HashMap, sync::Arc};

use rust_extensions::date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds};
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use crate::tcp_gateway::{TcpConnectionInner, TcpGatewayConnection, TcpGatewayContract};

use super::TcpGatewayServerForwardConnection;

pub struct TcpGatewayServerConnection {
    inner: Arc<TcpConnectionInner>,

    pub remote_addr: Arc<String>,
    pub gateway_id: Arc<String>,
    last_incoming_payload_time: AtomicDateTimeAsMicroseconds,
    forward_connections: Mutex<HashMap<u32, Arc<TcpGatewayServerForwardConnection>>>,
}

impl TcpGatewayServerConnection {
    pub fn new(
        gateway_id: Arc<String>,
        remote_addr: Arc<String>,
        connection: OwnedWriteHalf,
    ) -> Self {
        let (inner, receiver) = TcpConnectionInner::new(connection);

        let inner = Arc::new(inner);

        let result = Self {
            inner: inner.clone(),

            remote_addr,
            gateway_id,
            forward_connections: Mutex::default(),
            last_incoming_payload_time: AtomicDateTimeAsMicroseconds::now(),
        };

        super::super::tcp_connection_inner::start_write_loop(inner, receiver);

        result
    }
}

#[async_trait::async_trait]
impl TcpGatewayConnection for TcpGatewayServerConnection {
    type ForwardConnection = TcpGatewayServerForwardConnection;

    fn get_addr(&self) -> &str {
        self.remote_addr.as_str()
    }

    fn set_last_incoming_payload_time(&self, time: DateTimeAsMicroseconds) {
        self.last_incoming_payload_time.update(time);
    }

    fn get_last_incoming_payload_time(&self) -> DateTimeAsMicroseconds {
        self.last_incoming_payload_time.as_date_time()
    }

    async fn disconnect(&self) {
        self.inner.disconnect().await
    }
    async fn send_payload(&self, payload: &TcpGatewayContract) -> bool {
        let vec = payload.to_vec();
        self.inner.send_payload(vec.as_slice()).await
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
        let read_access = self.forward_connections.lock().await;
        read_access.get(&connection_id).cloned()
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
