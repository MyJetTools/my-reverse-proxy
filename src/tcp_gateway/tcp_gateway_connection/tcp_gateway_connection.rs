use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicI64},
        Arc,
    },
    time::Duration,
};

use encryption::aes::AesKey;

use rust_extensions::{
    date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds},
    remote_endpoint::RemoteEndpointOwned,
    AtomicDuration, AtomicStopWatch, SliceOrVec,
};
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use crate::metrics::PerSecondAccumulator;

use super::{super::forwarded_connection::*, super::*};

pub struct TcpGatewayConnection {
    gateway_id: Mutex<Arc<String>>,
    pub addr: Arc<String>,
    inner: Arc<TcpConnectionInner>,
    last_incoming_payload_time: AtomicDateTimeAsMicroseconds,
    forward_connections: Arc<Mutex<HashMap<u32, Arc<TcpGatewayForwardConnection>>>>,
    forward_proxy_handlers: Arc<Mutex<HashMap<u32, TcpGatewayProxyForwardConnectionHandler>>>,
    allow_incoming_forward_connection: bool,
    support_compression: AtomicBool,
    pub ping_stop_watch: AtomicStopWatch,
    pub last_ping_duration: AtomicDuration,
    pub file_requests: FileRequests,
    pub metrics: Mutex<GatewayConnectionMetrics>,
    pub in_per_second: PerSecondAccumulator,
    pub out_per_second: PerSecondAccumulator,
    connection_timestamp: AtomicI64,
    pub created_at: DateTimeAsMicroseconds,
}

impl TcpGatewayConnection {
    pub fn new(
        addr: Arc<String>,
        write_half: OwnedWriteHalf,
        aes_key: Arc<AesKey>,
        supported_compression: bool,
        allow_incoming_forward_connection: bool,
    ) -> Self {
        let (inner, receiver) = TcpConnectionInner::new(write_half, aes_key);
        let inner = Arc::new(inner);
        let result = Self {
            gateway_id: Mutex::new(Arc::new(String::new())),
            addr,
            inner: inner.clone(),
            forward_connections: Arc::new(Mutex::default()),
            forward_proxy_handlers: Arc::new(Mutex::default()),
            last_incoming_payload_time: AtomicDateTimeAsMicroseconds::now(),
            support_compression: AtomicBool::new(supported_compression),
            ping_stop_watch: AtomicStopWatch::new(),
            last_ping_duration: AtomicDuration::from_micros(0),
            file_requests: FileRequests::new(),
            allow_incoming_forward_connection,
            metrics: Mutex::default(),
            in_per_second: PerSecondAccumulator::new(),
            out_per_second: PerSecondAccumulator::new(),
            connection_timestamp: AtomicI64::new(0),
            created_at: DateTimeAsMicroseconds::now(),
        };

        super::super::tcp_connection_inner::start_write_loop(inner, receiver);

        result
    }

    pub fn set_connection_timestamp(&self, value: DateTimeAsMicroseconds) {
        self.connection_timestamp.store(
            value.unix_microseconds,
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    pub fn get_connection_timestamp(&self) -> DateTimeAsMicroseconds {
        let result = self
            .connection_timestamp
            .load(std::sync::atomic::Ordering::Relaxed);

        DateTimeAsMicroseconds::new(result)
    }

    pub fn has_handshake(&self) -> bool {
        let result = self
            .connection_timestamp
            .load(std::sync::atomic::Ordering::Relaxed);

        result > 0
    }

    pub async fn one_second_timer_tick(&self) {
        let in_per_second = self.in_per_second.get_per_second();
        let out_per_second = self.out_per_second.get_per_second();

        let mut write_access = self.metrics.lock().await;
        write_access.in_per_second.add(in_per_second);
        write_access.out_per_second.add(out_per_second);
    }

    pub fn get_aes_key(&self) -> &Arc<AesKey> {
        &self.inner.aes_key
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

    pub async fn connect_to_forward_proxy_connection(
        &self,
        remote_endpoint: Arc<RemoteEndpointOwned>,
        timeout: Duration,
        connection_id: u32,
    ) -> Result<TcpGatewayProxyForwardStream, String> {
        if !self.has_handshake() {
            println!("No Handshake");
            return Err(format!(
                "Failed establishing connection to {}. Reason: Tcp gateway connection created at {} did not do handshake yet",
                remote_endpoint.as_str(),
                self.created_at.to_rfc3339()
            ));
        }

        let gateway_id = self.get_gateway_id().await;

        println!(
            "Connecting to {}->{} with timeout {:?} and id {}",
            gateway_id,
            remote_endpoint.as_str(),
            timeout,
            connection_id
        );

        let connection = TcpGatewayProxyForwardConnectionHandler::new(
            connection_id,
            self.inner.clone(),
            self.get_supported_compression(),
        );

        {
            let mut write_access = self.forward_proxy_handlers.lock().await;
            write_access.insert(connection_id, connection);
        }

        let remote_host_port = remote_endpoint.get_host_port();

        let connect_contract = TcpGatewayContract::Connect {
            connection_id,
            timeout,
            remote_host: remote_host_port.as_str(),
        };

        if !self.send_payload(&connect_contract).await {
            return Err(format!(
                "Connection to endpoint {} is lost",
                remote_host_port.as_str()
            ));
        }

        match self
            .ack_connection(remote_host_port.as_str(), connection_id)
            .await
        {
            Ok(result) => Ok(result),
            Err(err) => {
                let mut connections_access = self.forward_proxy_handlers.lock().await;
                connections_access.remove(&connection_id);
                Err(err)
            }
        }
    }

    async fn ack_connection(
        &self,
        remote_host_port: &str,
        connection_id: u32,
    ) -> Result<TcpGatewayProxyForwardStream, String> {
        loop {
            if !self.is_gateway_connected() {
                return Err(format!(
                    "Cannot connect to {}. Gateway connection is lost",
                    remote_host_port
                ));
            }

            tokio::time::sleep(Duration::from_millis(100)).await;

            let connections_access = self.forward_proxy_handlers.lock().await;
            let connection = connections_access.get(&connection_id);

            if connection.is_none() {
                return Err(format!(
                    "Connection to {} is somehow closed",
                    remote_host_port
                ));
            }

            let connection = connection.unwrap();

            match connection.get_status() {
                TcpGatewayProxyForwardedConnectionStatus::AcknowledgingConnection => {}
                TcpGatewayProxyForwardedConnectionStatus::Connected => {
                    return Ok(connection.get_connection());
                }
                TcpGatewayProxyForwardedConnectionStatus::Disconnected(err) => {
                    return Err(err.to_string());
                }
            }
        }
    }

    pub async fn notify_forward_proxy_connection_accepted(&self, connection_id: u32) {
        let mut write_access = self.forward_proxy_handlers.lock().await;

        if let Some(connection) = write_access.get_mut(&connection_id) {
            connection.set_connected();
        }
    }

    pub async fn request_file(&self, path: &str) -> Result<Vec<u8>, FileRequestError> {
        if !self.has_handshake() {
            return Err(FileRequestError::GatewayDisconnected);
        }

        let (task_completion, request_id) = self.file_requests.start_request().await;

        {
            let request = TcpGatewayContract::GetFileRequest { path, request_id };
            self.send_payload(&request).await;
        }

        task_completion.get_result().await
    }

    pub async fn notify_file_response(
        &self,
        request_id: u32,
        status: GetFileStatus,
        content: SliceOrVec<'_, u8>,
    ) {
        match status {
            GetFileStatus::Ok => {
                self.file_requests
                    .set_content(request_id, content.into_vec())
                    .await;
            }
            GetFileStatus::Error => {
                self.file_requests
                    .set_error(request_id, FileRequestError::FileNotFound)
                    .await
            }
        }
    }

    pub async fn disconnect_gateway(&self) {
        self.inner.disconnect().await;
    }

    pub fn is_gateway_connected(&self) -> bool {
        self.inner.is_connected()
    }

    pub fn is_incoming_forward_connection_allowed(&self) -> bool {
        self.allow_incoming_forward_connection
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

    pub async fn incoming_payload_for_proxy_connection(&self, connection_id: u32, payload: &[u8]) {
        let mut proxy_connection_access = self.forward_proxy_handlers.lock().await;

        let proxy_connection = proxy_connection_access.get_mut(&connection_id);

        if proxy_connection.is_none() {
            let gateway_id = self.get_gateway_id().await;
            println!(
                "Gateway:[{}]. Proxy connection with id {} not found",
                gateway_id, connection_id
            );
            return;
        }

        let proxy_connection = proxy_connection.unwrap();

        let status = proxy_connection.get_status();

        match status {
            TcpGatewayProxyForwardedConnectionStatus::AcknowledgingConnection => {
                let gateway_id = self.get_gateway_id().await;
                println!("Gateway:[{}] Can not accept payload with size: {} to connection {}. Connection is not acknowledged yet", gateway_id.as_str(), payload.len(), connection_id);
            }
            TcpGatewayProxyForwardedConnectionStatus::Connected => {
                proxy_connection.enqueue_receive_payload(payload);
            }
            TcpGatewayProxyForwardedConnectionStatus::Disconnected(err) => {
                let gateway_id = self.get_gateway_id().await;
                println!("Gateway:[{}] Can not accept payload with size: {} to connection {}. Connection is disconnected with err: {}", gateway_id.as_str(), payload.len(), connection_id,  err);
            }
        }
    }

    pub async fn send_payload<'d>(&self, payload: &TcpGatewayContract<'d>) -> bool {
        let supported_compression = self.get_supported_compression();
        let vec = payload.to_vec(&self.inner.aes_key, supported_compression);

        if self.inner.send_payload(vec.as_slice()).await {
            self.out_per_second.add(vec.len());
            return true;
        }

        false
    }

    pub fn set_last_incoming_payload_time(&self, time: DateTimeAsMicroseconds) {
        self.last_incoming_payload_time.update(time);
    }

    pub fn get_last_incoming_payload_time(&self) -> DateTimeAsMicroseconds {
        self.last_incoming_payload_time.as_date_time()
    }

    pub async fn get_forward_proxy_connections_amount(&self) -> usize {
        let write_access = self.forward_proxy_handlers.lock().await;
        write_access.len()
    }

    pub async fn disconnect_forward_proxy_connection(&self, connection_id: u32, message: &str) {
        let mut write_access = self.forward_proxy_handlers.lock().await;
        if let Some(mut connection) = write_access.remove(&connection_id) {
            connection.set_connection_error(message.into());
        }
    }
}

impl Drop for TcpGatewayConnection {
    fn drop(&mut self) {
        let proxy_connections = self.forward_proxy_handlers.clone();
        let forward_connections = self.forward_connections.clone();
        tokio::spawn(async move {
            {
                let write_access = proxy_connections.lock().await;
                for itm in write_access.values() {
                    itm.disconnect();
                }
            }

            {
                let write_access = forward_connections.lock().await;
                for itm in write_access.values() {
                    itm.disconnect().await;
                }
            }
        });
    }
}
