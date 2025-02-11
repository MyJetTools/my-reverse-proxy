use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use encryption::aes::AesKey;

use rust_extensions::{
    date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds},
    remote_endpoint::RemoteEndpointOwned,
    AtomicDuration, AtomicStopWatch, SliceOrVec,
};
use tokio::{net::tcp::OwnedWriteHalf, sync::Mutex};

use super::{super::forwarded_connection::*, super::*};

pub struct TcpGatewayConnection {
    gateway_id: Mutex<Arc<String>>,
    pub addr: Arc<String>,
    inner: Arc<TcpConnectionInner>,
    last_incoming_payload_time: AtomicDateTimeAsMicroseconds,
    forward_connections: Mutex<HashMap<u32, Arc<TcpGatewayForwardConnection>>>,
    forward_proxy_connections: Mutex<HashMap<u32, TcpGatewayProxyForwardConnectionHandler>>,
    support_compression: AtomicBool,
    pub ping_stop_watch: AtomicStopWatch,
    pub last_ping_duration: AtomicDuration,
    pub file_requests: FileRequests,
}

impl TcpGatewayConnection {
    pub fn new(
        addr: Arc<String>,
        write_half: OwnedWriteHalf,
        aes_key: Arc<AesKey>,
        supported_connection: bool,
    ) -> Self {
        let (inner, receiver) = TcpConnectionInner::new(write_half, aes_key);
        let inner = Arc::new(inner);
        let result = Self {
            gateway_id: Mutex::new(Arc::new(String::new())),
            addr,
            inner: inner.clone(),
            forward_connections: Mutex::default(),
            forward_proxy_connections: Mutex::default(),
            last_incoming_payload_time: AtomicDateTimeAsMicroseconds::now(),
            support_compression: AtomicBool::new(supported_connection),
            ping_stop_watch: AtomicStopWatch::new(),
            last_ping_duration: AtomicDuration::from_micros(0),
            file_requests: FileRequests::new(),
        };

        super::super::tcp_connection_inner::start_write_loop(inner, receiver);

        result
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

    pub async fn disconnect_forward_connection(&self, connection_id: u32) {
        if let Some(forward_connection) = self.remove_forward_connection(connection_id).await {
            forward_connection.disconnect().await;
        }
    }

    pub async fn connect_to_forward_proxy_connection(
        &self,
        remote_endpoint: Arc<RemoteEndpointOwned>,
        timeout: Duration,
        connection_id: u32,
    ) -> Result<TcpGatewayProxyForwardStream, String> {
        let gateway_id = self.get_gateway_id().await;

        let connection = TcpGatewayProxyForwardConnectionHandler::new(
            connection_id,
            self.inner.clone(),
            self.get_supported_compression(),
        );

        {
            let mut write_access = self.forward_proxy_connections.lock().await;
            write_access.insert(connection_id, connection);
        }

        let connect_contract = TcpGatewayContract::Connect {
            connection_id,
            timeout,
            remote_host: remote_endpoint.as_str(),
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

            let connections_access = self.forward_proxy_connections.lock().await;

            let connection = connections_access.get(&connection_id);

            if connection.is_none() {
                panic!("Somehow no connection with id {}", connection_id);
            }

            let connection = connection.unwrap();

            match connection.get_status() {
                TcpGatewayProxyForwardedConnectionStatus::AwaitingConnection => {}
                TcpGatewayProxyForwardedConnectionStatus::Connected => {
                    return Ok(connection.get_connection());
                }
                TcpGatewayProxyForwardedConnectionStatus::Disconnected(err) => {
                    return Err(err.as_str().to_string());
                }
            }
        }

        return Err(format!(
            "Gateway {} connection to endpoint {} is lost during awaiting forward to {} connecting result",
            gateway_id,
            self.addr.as_str(),
            remote_endpoint.as_str()
        ));
    }

    pub async fn notify_forward_proxy_connection_accepted(&self, connection_id: u32) {
        let mut write_access = self.forward_proxy_connections.lock().await;

        if let Some(connection) = write_access.get_mut(&connection_id) {
            connection.set_connected();
        }
    }

    pub async fn request_file(&self, path: &str) -> Result<Vec<u8>, FileRequestError> {
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
        let mut proxy_connection_access = self.forward_proxy_connections.lock().await;

        let proxy_connection = proxy_connection_access.get_mut(&connection_id);

        if proxy_connection.is_none() {
            println!("Proxy connection with id {} not found", connection_id);
            return;
        }

        let proxy_connection = proxy_connection.unwrap();

        let status = proxy_connection.get_status();

        match status {
            TcpGatewayProxyForwardedConnectionStatus::AwaitingConnection => {
                let gateway_id = self.get_gateway_id().await;
                println!("Can not accept payload with size: {} to connection {}  through gateway {}. Connection is not connected yet", payload.len(), connection_id, gateway_id.as_str());
            }
            TcpGatewayProxyForwardedConnectionStatus::Connected => {
                proxy_connection.enqueue_receive_payload(payload);
            }
            TcpGatewayProxyForwardedConnectionStatus::Disconnected(err) => {
                let gateway_id = self.get_gateway_id().await;
                println!("Can not accept payload with size: {} to connection {}  through gateway {}. Connection is disconnected with err: {}", payload.len(), connection_id, gateway_id.as_str(), err);
            }
        }
    }

    pub async fn send_payload<'d>(&self, payload: &TcpGatewayContract<'d>) -> bool {
        let supported_compression = self.get_supported_compression();
        let vec = payload.to_vec(&self.inner.aes_key, supported_compression);
        self.inner.send_payload(vec.as_slice()).await
    }

    pub fn set_last_incoming_payload_time(&self, time: DateTimeAsMicroseconds) {
        self.last_incoming_payload_time.update(time);
    }

    pub fn get_last_incoming_payload_time(&self) -> DateTimeAsMicroseconds {
        self.last_incoming_payload_time.as_date_time()
    }

    pub async fn get_forward_proxy_connections_amount(&self) -> usize {
        let write_access = self.forward_proxy_connections.lock().await;
        write_access.len()
    }

    pub async fn disconnect_forward_proxy_connection(&self, connection_id: u32, message: &str) {
        let mut write_access = self.forward_proxy_connections.lock().await;
        if let Some(mut connection) = write_access.remove(&connection_id) {
            connection.set_connection_error(message.into());
        }
    }
}
