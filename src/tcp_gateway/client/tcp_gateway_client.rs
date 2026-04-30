use std::{
    sync::{atomic::AtomicU32, Arc},
    time::Duration,
};

use ed25519_dalek::SigningKey;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::net::TcpStream;

use crate::tcp_gateway::handshake::perform_client_handshake;
use crate::tcp_gateway::{client::*, *};

pub struct TcpGatewayClient {
    inner: Arc<TcpGatewayInner>,
    next_connection_id: AtomicU32,
}

impl TcpGatewayClient {
    pub fn new(
        id: String,
        remote_endpoint: String,
        signing_key: SigningKey,
        compress_outbound: bool,
        allow_incoming_forward_connections: bool,
        connect_timeout: Duration,
        debug: bool,
        sync_ssl_certificates: Vec<String>,
    ) -> Self {
        let inner = Arc::new(TcpGatewayInner::new_client(
            id,
            remote_endpoint,
            signing_key,
            allow_incoming_forward_connections,
            sync_ssl_certificates,
            compress_outbound,
        ));
        let result = Self {
            inner: inner.clone(),
            next_connection_id: AtomicU32::new(0),
        };

        crate::app::spawn_named(
            "tcp_gateway_client_reconnect_loop",
            connection_loop(inner.clone(), connect_timeout, debug),
        );

        result
    }

    pub fn get_next_connection_id(&self) -> u32 {
        self.next_connection_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_gateway_connection(
        &self,
        gateway_id: &str,
    ) -> Option<Arc<TcpGatewayConnection>> {
        self.inner.get_gateway_connection(gateway_id)
    }

    pub fn get_gateway_connections(&self) -> Vec<Arc<TcpGatewayConnection>> {
        self.inner.get_gateway_connections()
    }

    pub async fn timer_1s(&self) {
        for connection in self.get_gateway_connections() {
            connection.one_second_timer_tick();
        }
    }

    pub fn get_sync_ssl_certificates(&self) -> &[String] {
        self.inner.sync_ssl_certificates.as_slice()
    }
}

impl Drop for TcpGatewayClient {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

async fn connection_loop(
    inner: Arc<TcpGatewayInner>,
    connect_timeout: Duration,
    debug: bool,
) {
    while inner.is_running() {
        inner.set_gateway_connection(&inner.gateway_id, None);
        println!(
            "Gateway:[{}] Connecting to remote gateway with host '{}' and with timeout: {:?}",
            inner.get_gateway_id(),
            inner.gateway_host.as_str(),
            connect_timeout
        );
        let connect_feature = TcpStream::connect(inner.gateway_host.as_str());

        let connect_result = tokio::time::timeout(connect_timeout, connect_feature).await;

        if connect_result.is_err() {
            println!(
                "Gateway:[{}] Can not connect to Gateway Server with host '{}'. Timeout: {:?}",
                inner.get_gateway_id(),
                inner.gateway_host.as_str(),
                connect_timeout
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        let mut tcp_stream = match connect_result.unwrap() {
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

        let signing_key = inner
            .signing_key
            .as_ref()
            .expect("Gateway client missing signing_key")
            .as_ref()
            .clone();

        let session_key = match perform_client_handshake(
            &mut tcp_stream,
            &signing_key,
            inner.gateway_id.as_str(),
        )
        .await
        {
            Ok(key) => key,
            Err(err) => {
                eprintln!(
                    "Gateway:[{}] handshake to '{}' failed: {err}",
                    inner.get_gateway_id(),
                    inner.gateway_host.as_str(),
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let (read, write) = tcp_stream.into_split();

        let gateway_connection = TcpGatewayConnection::new(
            inner.gateway_host.clone(),
            write.into(),
            session_key,
            inner.compress_outbound,
            inner.allow_incoming_forward_connections,
        );

        let gateway_connection = Arc::new(gateway_connection);
        let now = DateTimeAsMicroseconds::now();
        gateway_connection.set_connection_timestamp(now);
        gateway_connection.set_gateway_id(inner.gateway_id.as_str());
        inner.set_gateway_connection(&inner.gateway_id, Some(gateway_connection.clone()));

        crate::app::spawn_named(
            "tcp_gateway_client_read_loop",
            crate::tcp_gateway::gateway_read_loop(
                inner.clone(),
                read,
                gateway_connection.clone(),
                TcpGatewayClientPacketHandler::new(debug),
                debug,
            ),
        );

        let sync_ids: Vec<&str> = inner
            .sync_ssl_certificates
            .iter()
            .map(|s| s.as_str())
            .collect();
        if !sync_ids.is_empty() {
            let request = TcpGatewayContract::SyncSslCertificatesRequest { cert_ids: sync_ids };
            gateway_connection.send_payload(&request);
        }

        super::gateway_ping_loop(gateway_connection, debug).await;
    }
}
