use std::{sync::Arc, time::Duration};

use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::net::TcpStream;

use crate::tcp_gateway::{client::*, *};

pub struct TcpGatewayClient {
    inner: Arc<TcpGatewayInner>,
}

impl TcpGatewayClient {
    pub fn new(id: String, remote_endpoint: String) -> Self {
        let inner = Arc::new(TcpGatewayInner::new(id, remote_endpoint));
        let result = Self {
            inner: inner.clone(),
        };

        tokio::spawn(connection_loop(inner.clone()));

        result
    }
}

impl Drop for TcpGatewayClient {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

async fn connection_loop(inner: Arc<TcpGatewayInner>) {
    while inner.is_running() {
        println!(
            "Connecting to remote gateway '{}' with addr '{}'",
            inner.get_id(),
            inner.addr.as_str()
        );
        let tcp_stream = TcpStream::connect(inner.addr.as_str()).await;

        let tcp_stream = match tcp_stream {
            Ok(tcp_stream) => tcp_stream,
            Err(err) => {
                println!(
                    "Can not connect to remote gateway {}. Err: {:?}",
                    inner.get_id(),
                    err
                );

                tokio::time::sleep(Duration::from_secs(5)).await;

                continue;
            }
        };

        let (read, write) = tcp_stream.into_split();

        let gateway_client_connection =
            TcpGatewayClientConnection::new(inner.id.clone(), inner.addr.clone(), write);

        let gateway_client_connection = Arc::new(gateway_client_connection);

        tokio::spawn(crate::tcp_gateway::gateway_read_loop::read_loop(
            inner.clone(),
            read,
            gateway_client_connection.clone(),
            TcpGatewayClientPacketHandler::new(),
        ));

        let handshake_contract = TcpGatewayContract::Handshake {
            timestamp: DateTimeAsMicroseconds::now().unix_microseconds,
            client_name: inner.get_id(),
        };

        gateway_client_connection
            .send_payload(&handshake_contract)
            .await;

        super::ping_loop(gateway_client_connection).await;
    }
}
