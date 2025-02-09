use std::{
    sync::{atomic::AtomicU32, Arc},
    time::Duration,
};

use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::net::TcpStream;

use crate::tcp_gateway::{client::*, forwarded_connection::TcpGatewayProxyForwardedConnection, *};

const CLIENT_CONNECTION_ID: &str = "";

pub struct TcpGatewayClient {
    inner: Arc<TcpGatewayInner>,
    next_connection_id: AtomicU32,
}

impl TcpGatewayClient {
    pub fn new(id: String, remote_endpoint: String, debug: bool) -> Self {
        let inner = Arc::new(TcpGatewayInner::new(id, remote_endpoint));
        let result = Self {
            inner: inner.clone(),
            next_connection_id: AtomicU32::new(0),
        };

        tokio::spawn(connection_loop(inner.clone(), debug));

        result
    }

    fn get_next_connection_id(&self) -> u32 {
        self.next_connection_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn connect_to_forward_proxy_connection(
        &self,
        remote_endpoint: &str,
        debug: bool,
    ) -> Result<
        (
            Arc<TcpGatewayProxyForwardedConnection>,
            Arc<TcpGatewayConnection>,
        ),
        String,
    > {
        let gateway_connection = self
            .inner
            .get_gateway_connection(CLIENT_CONNECTION_ID)
            .await;

        if gateway_connection.is_none() {
            let err = format!(
                "Gateway {} connection to endpoint {} is not established",
                self.inner.get_id(),
                self.inner.addr.as_str()
            );

            println!("{}", err);
            return Err(err);
        }

        let gateway_connection = gateway_connection.unwrap();

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
            .await?;

        Ok((result, gateway_connection))
    }
}

impl Drop for TcpGatewayClient {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

async fn connection_loop(inner: Arc<TcpGatewayInner>, debug: bool) {
    while inner.is_running() {
        inner
            .set_gateway_connection(CLIENT_CONNECTION_ID, None)
            .await;
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

        let gateway_connection =
            TcpGatewayConnection::new(inner.id.clone(), inner.addr.clone(), write);

        let gateway_connection = Arc::new(gateway_connection);
        inner
            .set_gateway_connection(CLIENT_CONNECTION_ID, gateway_connection.clone().into())
            .await;

        tokio::spawn(crate::tcp_gateway::gateway_read_loop::read_loop(
            inner.clone(),
            read,
            gateway_connection.clone(),
            TcpGatewayClientPacketHandler::new(debug),
            debug,
        ));

        let handshake_contract = TcpGatewayContract::Handshake {
            timestamp: DateTimeAsMicroseconds::now().unix_microseconds,
            client_name: inner.get_id(),
        };

        gateway_connection.send_payload(&handshake_contract).await;

        super::gateway_ping_loop(gateway_connection, debug).await;
    }
}
