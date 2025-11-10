use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use encryption::aes::AesKey;
use tokio::net::TcpListener;

use super::super::*;
use super::*;

pub struct TcpGatewayServer {
    inner: Arc<TcpGatewayInner>,
    next_connection_id: AtomicU32,
}

impl TcpGatewayServer {
    pub fn new(listen: String, encryption: AesKey, debug: bool) -> Self {
        println!("Starting TCP Gateway Server at address: {}", listen);
        let inner = Arc::new(TcpGatewayInner::new(
            "ServerGateway".to_string(),
            listen,
            true,
            encryption,
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

        let (read, write) = tcp_stream.into_split();

        let tcp_gateway_connection = TcpGatewayConnection::new(
            tcp_gateway.gateway_host.clone(),
            write,
            tcp_gateway.encryption.clone(),
            true,
            false,
        );

        let tcp_gateway_connection = Arc::new(tcp_gateway_connection);

        tokio::spawn(crate::tcp_gateway::gateway_read_loop(
            tcp_gateway.clone(),
            read,
            tcp_gateway_connection,
            TcpGatewayServerPacketHandler,
            debug,
        ));
    }
}
