use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use encryption::aes::AesKey;

use rust_extensions::{
    date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds},
    AtomicDuration, AtomicStopWatch,
};
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use super::{
    forwarded_connection::{TcpGatewayForwardConnection, TcpGatewayProxyForwardedConnection},
    *,
};

pub struct TcpGatewayConnection {
    gateway_id: Mutex<Arc<String>>,
    pub addr: Arc<String>,
    inner: Arc<TcpConnectionInner>,
    last_incoming_payload_time: AtomicDateTimeAsMicroseconds,
    forward_connections: Mutex<HashMap<u32, Arc<TcpGatewayForwardConnection>>>,
    forward_proxy_connections: Mutex<HashMap<u32, Arc<TcpGatewayProxyForwardedConnection>>>,
    pub aes_key: Arc<AesKey>,
    support_compression: AtomicBool,
    pub ping_stop_watch: AtomicStopWatch,
    pub last_ping_duration: AtomicDuration,
}

impl TcpGatewayConnection {
    pub fn new(
        addr: Arc<String>,
        write_half: OwnedWriteHalf,
        aes_key: Arc<AesKey>,
        supported_connection: bool,
    ) -> Self {
        let (inner, receiver) = TcpConnectionInner::new(write_half);
        let inner = Arc::new(inner);
        let result = Self {
            gateway_id: Mutex::new(Arc::new(String::new())),
            addr,
            inner: inner.clone(),
            forward_connections: Mutex::default(),
            forward_proxy_connections: Mutex::default(),
            last_incoming_payload_time: AtomicDateTimeAsMicroseconds::now(),
            aes_key,
            support_compression: AtomicBool::new(supported_connection),
            ping_stop_watch: AtomicStopWatch::new(),
            last_ping_duration: AtomicDuration::from_micros(0),
        };

        super::tcp_connection_inner::start_write_loop(inner, receiver);

        result
    }

    pub fn get_supported_compression(&self) -> bool {
        self.support_compression
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_supported_compression(&self, value: bool) {
        self.support_compression
            .store(value, std::sync::atomic::Ordering::Relaxed);
    }

    pub async fn set_gateway_id(&self, id: &str) {
        let mut gateway_id = self.gateway_id.lock().await;
        *gateway_id = Arc::new(id.to_string());
    }

    pub async fn get_gateway_id(&self) -> Arc<String> {
        let gateway = self.gateway_id.lock().await;
        gateway.clone()
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

    pub async fn get_forward_connections_amount(&self) -> usize {
        let write_access = self.forward_connections.lock().await;
        write_access.len()
    }

    pub async fn remove_forward_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<TcpGatewayForwardConnection>> {
        let mut write_access = self.forward_connections.lock().await;
        write_access.remove(&connection_id)
    }

    pub async fn disconnect_forward_connection(&self, connection_id: u32) {
        if let Some(forward_connection) = self.remove_forward_connection(connection_id).await {
            forward_connection.disconnect().await;
        }
    }

    pub async fn connect_to_forward_proxy_connection(
        &self,
        remote_endpoint: &str,
        timeout: Duration,
        connection_id: u32,
    ) -> Result<Arc<TcpGatewayProxyForwardedConnection>, String> {
        let gateway_id = self.get_gateway_id().await;
        let connection = Arc::new(TcpGatewayProxyForwardedConnection::new(
            connection_id,
            gateway_id.clone(),
            self.inner.clone(),
            remote_endpoint.to_string(),
            self.get_supported_compression(),
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
                gateway_id.as_str(),
                self.addr.as_str()
            ));
        }

        while self.is_gateway_connected() {
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
            gateway_id,
            self.addr.as_str(),
            remote_endpoint
        ));
    }

    pub async fn notify_forward_proxy_connection_accepted(&self, connection_id: u32) {
        let connection = self.get_forward_proxy_connection(connection_id).await;

        if let Some(connection) = connection {
            connection.set_connected().await;
        }
    }

    pub async fn disconnect_gateway(&self) {
        self.inner.disconnect().await;
    }

    pub fn is_gateway_connected(&self) -> bool {
        self.inner.is_connected()
    }

    pub async fn send_backward_payload(&self, connection_id: u32, payload: &[u8]) {
        if self.has_forward_connection(connection_id).await {
            let payload = TcpGatewayContract::BackwardPayload {
                connection_id,
                payload: payload.into(),
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
                let gateway_id = self.get_gateway_id().await;
                println!("Can not accept payload with size: {} to connection {}  through gateway {}. Connection is not connected yet", payload.len(), connection_id, gateway_id.as_str());
            }
            forwarded_connection::TcpGatewayProxyConnectionStatus::Connected => {
                proxy_connection.enqueue_receive_payload(payload).await;
            }
            forwarded_connection::TcpGatewayProxyConnectionStatus::Disconnected(err) => {
                let gateway_id = self.get_gateway_id().await;
                println!("Can not accept payload with size: {} to connection {}  through gateway {}. Connection is disconnected with err: {}", payload.len(), connection_id, gateway_id.as_str(), err);
            }
        }
    }

    pub async fn send_payload<'d>(&self, payload: &TcpGatewayContract<'d>) -> bool {
        let supported_compression = self.get_supported_compression();
        let vec = payload.to_vec(&self.aes_key, supported_compression);
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

    pub async fn remove_forward_proxy_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<TcpGatewayProxyForwardedConnection>> {
        let mut write_access = self.forward_proxy_connections.lock().await;
        write_access.remove(&connection_id)
    }

    pub async fn get_forward_proxy_connections_amount(&self) -> usize {
        let write_access = self.forward_proxy_connections.lock().await;
        write_access.len()
    }

    pub async fn disconnect_forward_proxy_connection(&self, connection_id: u32, message: &str) {
        if let Some(connection) = self.remove_forward_proxy_connection(connection_id).await {
            connection.disconnect(message, &self.aes_key).await;
        }
    }
}
