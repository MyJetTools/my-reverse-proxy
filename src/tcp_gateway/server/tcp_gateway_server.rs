use std::sync::Arc;

use tokio::net::TcpListener;

use super::super::*;
use super::*;

pub struct TcpGatewayServer {
    inner: Arc<TcpGatewayInner>,
}

impl TcpGatewayServer {
    pub fn new(listen: String) -> Self {
        println!("Starting TCP Gateway Server at address: {}", listen);
        let inner = Arc::new(TcpGatewayInner::new("ServerGateway".to_string(), listen));
        let result = Self {
            inner: inner.clone(),
        };

        tokio::spawn(connection_loop(inner));

        result
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

async fn connection_loop(tcp_gateway: Arc<TcpGatewayInner>) {
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

        println!(
            "Gateway {} connection {} is accepted",
            tcp_gateway.get_id(),
            socket_addr
        );

        let (read, write) = tcp_stream.into_split();

        let tcp_gateway_connection =
            TcpGatewayConnection::new(tcp_gateway.id.clone(), tcp_gateway.addr.clone(), write);

        let tcp_gateway_connection = Arc::new(tcp_gateway_connection);

        tokio::spawn(crate::tcp_gateway::read_loop(
            tcp_gateway.clone(),
            read,
            tcp_gateway_connection,
            TcpGatewayServerPacketHandler,
        ));
    }
}
