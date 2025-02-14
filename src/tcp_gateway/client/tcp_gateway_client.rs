use std::{
    sync::{atomic::AtomicU32, Arc},
    time::Duration,
};

use encryption::aes::AesKey;
use tokio::net::TcpStream;

use crate::tcp_gateway::{client::*, *};

pub struct TcpGatewayClient {
    inner: Arc<TcpGatewayInner>,
    next_connection_id: AtomicU32,
}

impl TcpGatewayClient {
    pub fn new(
        id: String,
        remote_endpoint: String,
        encryption: AesKey,
        supported_compression: bool,
        allow_incoming_forward_connections: bool,
        debug: bool,
    ) -> Self {
        let inner = Arc::new(TcpGatewayInner::new(
            id,
            remote_endpoint,
            allow_incoming_forward_connections,
            encryption,
        ));
        let result = Self {
            inner: inner.clone(),
            next_connection_id: AtomicU32::new(0),
        };

        tokio::spawn(connection_loop(inner.clone(), supported_compression, debug));

        result
    }

    pub fn get_next_connection_id(&self) -> u32 {
        self.next_connection_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn get_gateway_connection(
        &self,
        gateway_id: &str,
    ) -> Option<Arc<TcpGatewayConnection>> {
        self.inner.get_gateway_connection(gateway_id).await
    }

    pub async fn get_gateway_connections(&self) -> Vec<Arc<TcpGatewayConnection>> {
        self.inner.get_gateway_connections().await
    }

    pub async fn timer_1s(&self) {
        for connection in self.get_gateway_connections().await {
            connection.one_second_timer_tick().await;
        }
    }
}

impl Drop for TcpGatewayClient {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

async fn connection_loop(inner: Arc<TcpGatewayInner>, supported_compression: bool, debug: bool) {
    while inner.is_running() {
        inner.set_gateway_connection(&inner.gateway_id, None).await;
        println!(
            "Gateway:[{}] Connecting to remote gateway with host '{}'",
            inner.get_gateway_id(),
            inner.gateway_host.as_str()
        );
        let tcp_stream = TcpStream::connect(inner.gateway_host.as_str()).await;

        let tcp_stream = match tcp_stream {
            Ok(tcp_stream) => tcp_stream,
            Err(err) => {
                println!(
                    "Gateway[{}]. Can not connect. Err: {:?}",
                    inner.get_gateway_id(),
                    err
                );

                tokio::time::sleep(Duration::from_secs(5)).await;

                continue;
            }
        };

        let (read, write) = tcp_stream.into_split();

        let gateway_connection = TcpGatewayConnection::new(
            inner.gateway_host.clone(),
            write,
            inner.encryption.clone(),
            supported_compression,
            inner.allow_incoming_forward_connections,
        );

        let gateway_connection = Arc::new(gateway_connection);
        inner
            .set_gateway_connection(&inner.gateway_id, gateway_connection.clone().into())
            .await;

        tokio::spawn(crate::tcp_gateway::gateway_read_loop::read_loop(
            inner.clone(),
            read,
            gateway_connection.clone(),
            TcpGatewayClientPacketHandler::new(debug),
            debug,
        ));

        println!(
            "Gateway: [{}] Sending handshake with timestamp {}",
            inner.gateway_id.as_str(),
            gateway_connection.created_at.unix_microseconds
        );

        let handshake_contract = TcpGatewayContract::Handshake {
            timestamp: gateway_connection.created_at.unix_microseconds,
            support_compression: supported_compression,
            gateway_name: inner.get_gateway_id(),
        };

        gateway_connection.send_payload(&handshake_contract).await;

        super::gateway_ping_loop(gateway_connection, debug).await;
    }
}
