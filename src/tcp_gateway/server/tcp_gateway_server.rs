use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;

use crate::tcp_gateway::forwarded_connection::TcpGatewayProxyForwardedConnection;

use super::super::*;
use super::*;

pub struct TcpGatewayServer {
    inner: Arc<TcpGatewayInner>,
    next_connection_id: AtomicU32,
}

impl TcpGatewayServer {
    pub fn new(listen: String, debug: bool) -> Self {
        println!("Starting TCP Gateway Server at address: {}", listen);
        let inner = Arc::new(TcpGatewayInner::new("ServerGateway".to_string(), listen));
        let result = Self {
            inner: inner.clone(),
            next_connection_id: AtomicU32::new(0),
        };

        tokio::spawn(connection_loop(inner, debug));

        result
    }

    fn get_next_connection_id(&self) -> u32 {
        self.next_connection_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn connect_to_forward_proxy_connection(
        &self,
        gateway_id: &str,
        remote_endpoint: &str,
        debug: bool,
    ) -> Option<(
        Arc<TcpGatewayProxyForwardedConnection>,
        Arc<TcpGatewayConnection>,
    )> {
        let gateway_connection = self.inner.get_gateway_connection(gateway_id).await?;

        let connection_id = self.get_next_connection_id();

        if debug {
            println!(
                "Connecting to {} with id {} ",
                remote_endpoint, connection_id
            );
        }

        let result = gateway_connection
            .connect_to_forward_proxy_connection(
                remote_endpoint,
                Duration::from_secs(5),
                connection_id,
            )
            .await;

        if result.is_err() {
            return None;
        }

        Some((result.unwrap(), gateway_connection))
    }
}

impl Drop for TcpGatewayServer {
    fn drop(&mut self) {
        println!(
            "Stopping TCP Gateway Server at address: {}",
            self.inner.addr
        );
        self.inner.stop();
    }
}

async fn connection_loop(tcp_gateway: Arc<TcpGatewayInner>, debug: bool) {
    let listener = TcpListener::bind(tcp_gateway.addr.as_str()).await;

    if let Err(err) = &listener {
        panic!(
            "Failed to start listening socket to serve TCP Gateway at address: {}. Err: {:?}",
            tcp_gateway.addr, err
        );
    }

    let listener = listener.unwrap();

    while tcp_gateway.is_running() {
        let accept_result = listener.accept().await;

        let (tcp_stream, socket_addr) = match accept_result {
            Ok(value) => value,
            Err(err) => {
                println!(
                    "Failed to accept connection at {} for. Err: {:?}",
                    tcp_gateway.addr.as_str(),
                    err
                );
                continue;
            }
        };

        if debug {
            println!(
                "Gateway {} connection {} is accepted",
                tcp_gateway.get_id(),
                socket_addr
            );
        }

        let (read, write) = tcp_stream.into_split();

        let tcp_gateway_connection =
            TcpGatewayConnection::new(tcp_gateway.id.clone(), tcp_gateway.addr.clone(), write);

        let tcp_gateway_connection = Arc::new(tcp_gateway_connection);

        tokio::spawn(crate::tcp_gateway::read_loop(
            tcp_gateway.clone(),
            read,
            tcp_gateway_connection,
            TcpGatewayServerPacketHandler::new(debug),
            debug,
        ));
    }
}
