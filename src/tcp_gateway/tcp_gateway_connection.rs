use std::{collections::HashMap, sync::Arc, time::Duration};

use rust_extensions::date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds};
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use super::{
    forwarded_connection::{TcpGatewayForwardConnection, TcpGatewayProxyForwardedConnection},
    *,
};

pub struct TcpGatewayConnection {
    pub gateway_id: Arc<String>,
    pub addr: Arc<String>,
    inner: Arc<TcpConnectionInner>,
    last_incoming_payload_time: AtomicDateTimeAsMicroseconds,
    forward_connections: Mutex<HashMap<u32, Arc<TcpGatewayForwardConnection>>>,
    forward_proxy_connections: Mutex<HashMap<u32, Arc<TcpGatewayProxyForwardedConnection>>>,
}

impl TcpGatewayConnection {
    pub fn new(gateway_id: Arc<String>, addr: Arc<String>, write_half: OwnedWriteHalf) -> Self {
        let (inner, receiver) = TcpConnectionInner::new(write_half);
        let inner = Arc::new(inner);
        let result = Self {
            gateway_id,
            addr,
            inner: inner.clone(),
            forward_connections: Mutex::default(),
            forward_proxy_connections: Mutex::default(),
            last_incoming_payload_time: AtomicDateTimeAsMicroseconds::now(),
        };

        super::tcp_connection_inner::start_write_loop(inner, receiver);

        result
    }

    pub async fn add_forward_connection(
        &self,
        connection_id: u32,
        connection: Arc<TcpGatewayForwardConnection>,
    ) {
        let mut write_access = self.forward_connections.lock().await;
        write_access.insert(connection_id, connection);
    }

    pub async fn get_forward_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<TcpGatewayForwardConnection>> {
        let write_access = self.forward_connections.lock().await;
        write_access.get(&connection_id).cloned()
    }

    pub async fn has_forward_connection(&self, connection_id: u32) -> bool {
        let read_access = self.forward_connections.lock().await;
        read_access.contains_key(&connection_id)
    }

    pub async fn remove_forward_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<TcpGatewayForwardConnection>> {
        let mut write_access = self.forward_connections.lock().await;
        write_access.remove(&connection_id)
    }

    pub async fn disconnect_forward_connection(&self, connection_id: u32) {
        self.remove_forward_connection(connection_id).await;
        //todo!("Думаю надо отправить disconnect payload");
    }

    pub async fn connect_forward_connection(
        &self,
        remote_endpoint: &str,
        timeout: Duration,
        connection_id: u32,
    ) -> Result<Arc<TcpGatewayProxyForwardedConnection>, String> {
        let connection = Arc::new(TcpGatewayProxyForwardedConnection::new(
            connection_id,
            self.gateway_id.clone(),
            self.inner.clone(),
            remote_endpoint.to_string(),
        ));
        {
            let mut write_access = self.forward_proxy_connections.lock().await;

            write_access.insert(connection_id, connection.clone());
        }

        let connect_contract = TcpGatewayContract::Connect {
            connection_id,
            timeout,
            remote_host: remote_endpoint,
        };

        if !self.send_payload(&connect_contract).await {
            return Err(format!(
                "Gateway {} connection to endpoint {} is lost",
                self.gateway_id,
                self.addr.as_str()
            ));
        }

        while self.is_connected() {
            tokio::time::sleep(Duration::from_millis(100)).await;

            match connection.get_status().await {
                forwarded_connection::TcpGatewayProxyConnectionStatus::AwaitingConnection => {}
                forwarded_connection::TcpGatewayProxyConnectionStatus::Connected => {
                    return Ok(connection);
                }
                forwarded_connection::TcpGatewayProxyConnectionStatus::Disconnected(err) => {
                    return Err(err);
                }
            }
        }

        return Err(format!(
            "Gateway {} connection to endpoint {} is lost during awaiting forward to {} connecting result",
            self.gateway_id,
            self.addr.as_str(),
            remote_endpoint
        ));
    }

    pub async fn notify_proxy_connection_accepted(&self, connection_id: u32) {
        let connection = self.get_forward_proxy_connection(connection_id).await;

        if let Some(connection) = connection {
            connection.set_connected().await;
        }
    }
    pub async fn notify_proxy_connection_disconnected(&self, connection_id: u32, err: &str) {
        let connection = self.remove_forward_proxy_connection(connection_id).await;

        if let Some(connection) = connection {
            connection.set_disconnected(err).await;
        }
    }

    pub async fn disconnect(&self) {
        self.inner.disconnect().await;
    }

    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    pub async fn send_backward_payload(&self, connection_id: u32, payload: &[u8]) {
        if self.has_forward_connection(connection_id).await {
            let payload = TcpGatewayContract::BackwardPayload {
                connection_id,
                payload,
            };
            self.send_payload(&payload).await;
        }
    }

    pub async fn notify_incoming_payload(&self, connection_id: u32, payload: &[u8]) {
        let proxy_connection = self.get_forward_proxy_connection(connection_id).await;

        if proxy_connection.is_none() {
            return;
        }

        let proxy_connection = proxy_connection.unwrap();

        let status = proxy_connection.get_status().await;

        match status {
            forwarded_connection::TcpGatewayProxyConnectionStatus::AwaitingConnection => {
                println!("Can not accept payload with size: {} to connection {}  through gateway {}. Connection is not connected yet", payload.len(), connection_id, self.gateway_id.as_str());
            }
            forwarded_connection::TcpGatewayProxyConnectionStatus::Connected => {
                proxy_connection.enqueue_receive_payload(payload).await;
            }
            forwarded_connection::TcpGatewayProxyConnectionStatus::Disconnected(err) => {
                println!("Can not accept payload with size: {} to connection {}  through gateway {}. Connection is disconnected with err: {}", payload.len(), connection_id, self.gateway_id.as_str(), err);
            }
        }
    }

    pub async fn send_payload<'d>(&self, payload: &TcpGatewayContract<'d>) -> bool {
        let vec = payload.to_vec();
        self.inner.send_payload(vec.as_slice()).await
    }

    pub fn set_last_incoming_payload_time(&self, time: DateTimeAsMicroseconds) {
        self.last_incoming_payload_time.update(time);
    }

    pub fn get_last_incoming_payload_time(&self) -> DateTimeAsMicroseconds {
        self.last_incoming_payload_time.as_date_time()
    }

    async fn get_forward_proxy_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<TcpGatewayProxyForwardedConnection>> {
        let write_access = self.forward_proxy_connections.lock().await;
        write_access.get(&connection_id).cloned()
    }

    async fn remove_forward_proxy_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<TcpGatewayProxyForwardedConnection>> {
        let mut write_access = self.forward_proxy_connections.lock().await;
        write_access.remove(&connection_id)
    }
}
