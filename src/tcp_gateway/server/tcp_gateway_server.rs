use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use ed25519_dalek::VerifyingKey;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::net::{TcpListener, TcpStream};

use super::super::*;
use super::*;
use crate::tcp_gateway::handshake::perform_server_handshake;

pub struct TcpGatewayServer {
    inner: Arc<TcpGatewayInner>,
    next_connection_id: AtomicU32,
}

impl TcpGatewayServer {
    pub fn new(
        listen: String,
        authorized_keys: Vec<VerifyingKey>,
        compress_outbound: bool,
        debug: bool,
    ) -> Self {
        println!("Starting TCP Gateway Server at address: {}", listen);
        let inner = Arc::new(TcpGatewayInner::new_server(
            "ServerGateway".to_string(),
            listen,
            authorized_keys,
            compress_outbound,
        ));
        let result = Self {
            inner: inner.clone(),
            next_connection_id: AtomicU32::new(0),
        };

        tokio::spawn(connection_loop(inner, debug));

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
}

impl Drop for TcpGatewayServer {
    fn drop(&mut self) {
        println!(
            "Stopping TCP Gateway Server at address: {}",
            self.inner.gateway_host
        );
        self.inner.stop();
    }
}

async fn connection_loop(tcp_gateway: Arc<TcpGatewayInner>, debug: bool) {
    let listener = TcpListener::bind(tcp_gateway.gateway_host.as_str()).await;

    let listener = match listener {
        Ok(listener) => listener,
        Err(err) => {
            panic!(
                "Failed to start listening socket to serve TCP Gateway at address: {}. Err: {:?}",
                tcp_gateway.gateway_host, err
            );
        }
    };

    while tcp_gateway.is_running() {
        let accept_result = listener.accept().await;

        let (tcp_stream, socket_addr) = match accept_result {
            Ok(value) => value,
            Err(err) => {
                println!(
                    "Failed to accept connection at {} for. Err: {:?}",
                    tcp_gateway.gateway_host.as_str(),
                    err
                );
                continue;
            }
        };

        if debug {
            println!(
                "Gateway {} connection {} is accepted",
                tcp_gateway.get_gateway_id(),
                socket_addr
            );
        }

        let tcp_gateway_clone = tcp_gateway.clone();
        tokio::spawn(handle_inbound(tcp_gateway_clone, tcp_stream, debug));
    }
}

async fn handle_inbound(tcp_gateway: Arc<TcpGatewayInner>, mut stream: TcpStream, debug: bool) {
    let authorized_keys = match tcp_gateway.authorized_keys.as_ref() {
        Some(keys) => keys.clone(),
        None => {
            eprintln!("Gateway server: no authorized_keys configured, dropping inbound");
            return;
        }
    };

    let outcome = match perform_server_handshake(&mut stream, authorized_keys.as_slice()).await {
        Ok(outcome) => outcome,
        Err(err) => {
            eprintln!(
                "Gateway server handshake failed at {}: {err}",
                tcp_gateway.gateway_host.as_str()
            );
            return;
        }
    };

    if debug {
        println!(
            "Gateway server handshake ok with client '{}' (timestamp_us={})",
            outcome.gateway_name, outcome.timestamp_us
        );
    }

    let (read, write) = stream.into_split();

    let gateway_connection = TcpGatewayConnection::new(
        tcp_gateway.gateway_host.clone(),
        write.into(),
        outcome.session_key.clone(),
        tcp_gateway.compress_outbound,
        true,
    );

    let gateway_connection = Arc::new(gateway_connection);
    let timestamp = DateTimeAsMicroseconds::new(outcome.timestamp_us);
    gateway_connection.set_connection_timestamp(timestamp);
    gateway_connection.set_gateway_id(outcome.gateway_name.as_str());
    tcp_gateway.set_gateway_connection(
        outcome.gateway_name.as_str(),
        Some(gateway_connection.clone()),
    );

    tokio::spawn(crate::tcp_gateway::gateway_read_loop(
        tcp_gateway,
        read,
        gateway_connection,
        TcpGatewayServerPacketHandler,
        debug,
    ));
}
