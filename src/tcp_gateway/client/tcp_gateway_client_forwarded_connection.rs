use std::{sync::Arc, time::Duration};

use tokio::{
    io::AsyncReadExt,
    net::{tcp::OwnedReadHalf, TcpStream},
};

use crate::tcp_gateway::*;

use super::TcpGatewayClientConnection;

pub struct TcpGatewayClientForwardConnection {
    gateway_connection: Arc<TcpGatewayClientConnection>,
    addr_id: Arc<String>,
    inner: Arc<TcpConnectionInner>,
    connection_id: u32,
}

impl TcpGatewayClientForwardConnection {
    pub async fn connect(
        connection_id: u32,
        gateway_connection: Arc<TcpGatewayClientConnection>,
        addr_id: Arc<String>,
        timeout: Duration,
    ) -> Result<Self, String> {
        let connect_future = TcpStream::connect(addr_id.as_str());

        let result = tokio::time::timeout(timeout, connect_future).await;

        if result.is_err() {
            return Err(format!(
                "Can not connect to {} using gateway {}. Err: Timeout {:?}",
                addr_id.as_str(),
                gateway_connection.gateway_id.as_str(),
                timeout
            ));
        }

        let result = result.unwrap();

        let tcp_stream = match result {
            Ok(tcp_stream) => tcp_stream,
            Err(err) => {
                return Err(format!(
                    "Can not connect to {} using gateway {}. Err: {:?}",
                    addr_id.as_str(),
                    gateway_connection.gateway_id.as_str(),
                    err
                ));
            }
        };

        let (read, write) = tcp_stream.into_split();

        // let mut tcp_connection_access = self.tcp_connection.lock().await;
        // *tcp_connection_access = Some(write);

        let (inner, receiver) = TcpConnectionInner::new(write);

        let inner = Arc::new(inner);

        let result = Self {
            gateway_connection: gateway_connection.clone(),
            addr_id,
            inner: inner.clone(),
            connection_id,
        };

        super::super::tcp_connection_inner::start_write_loop(inner.clone(), receiver);

        tokio::spawn(read_loop(read, gateway_connection, inner, connection_id));
        Ok(result)
    }
}

#[async_trait::async_trait]
impl TcpGatewayForwardConnection for TcpGatewayClientForwardConnection {
    fn get_addr(&self) -> &str {
        &self.addr_id
    }
    async fn disconnect(&self) {
        self.inner.disconnect().await
    }
    async fn send_payload(&self, payload: &[u8]) -> bool {
        self.inner.send_payload(payload).await
    }
}

async fn read_loop(
    mut read: OwnedReadHalf,
    gateway_connection: Arc<TcpGatewayClientConnection>,
    write: Arc<TcpConnectionInner>,
    connection_id: u32,
) {
    let mut buf = super::super::create_read_loop();

    loop {
        let read_size = match read.read(&mut buf).await {
            Ok(read_size) => read_size,
            Err(err) => {
                write.disconnect().await;
                let err = format!(
                    "ReadLoop. Can not read from connection {connection_id} using gateway {}  ConnectionError: {:?}",
                    gateway_connection.gateway_id.as_str(),
                    err
                );
                super::packet_handlers::send_connection_error(
                    &gateway_connection,
                    connection_id,
                    err.as_str(),
                    true,
                );
                break;
            }
        };

        if read_size == 0 {
            let err = format!(
                "ReadLoop. Connection {connection_id} using gateway {} id disconnected",
                gateway_connection.gateway_id.as_str(),
            );

            super::packet_handlers::send_connection_error(
                &gateway_connection,
                connection_id,
                err.as_str(),
                false,
            )
            .await;
            break;
        }

        let buffer = &buf[..read_size];

        super::super::send_payload_to_gateway(&gateway_connection, connection_id, buffer).await;
    }
}
