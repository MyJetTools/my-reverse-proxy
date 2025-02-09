use std::{sync::Arc, time::Duration};

use tokio::{
    io::AsyncReadExt,
    net::{tcp::OwnedReadHalf, TcpStream},
};

use crate::tcp_gateway::*;

pub struct TcpGatewayForwardConnection {
    pub remote_endpoint: Arc<String>,
    inner: Arc<TcpConnectionInner>,
}

impl TcpGatewayForwardConnection {
    pub async fn connect(
        connection_id: u32,
        gateway_connection: Arc<TcpGatewayConnection>,
        remote_endpoint: Arc<String>,
        timeout: Duration,
    ) -> Result<Self, String> {
        let connect_future = TcpStream::connect(remote_endpoint.as_str());

        let result = tokio::time::timeout(timeout, connect_future).await;

        if result.is_err() {
            return Err(format!(
                "Can not connect to {} using gateway {}. Err: Timeout {:?}",
                remote_endpoint.as_str(),
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
                    remote_endpoint.as_str(),
                    gateway_connection.gateway_id.as_str(),
                    err
                ));
            }
        };

        let (read, write) = tcp_stream.into_split();

        let (inner, receiver) = TcpConnectionInner::new(write);

        let inner = Arc::new(inner);

        let result = Self {
            remote_endpoint,
            inner: inner.clone(),
        };

        super::super::tcp_connection_inner::start_write_loop(inner.clone(), receiver);

        tokio::spawn(read_loop(read, gateway_connection, inner, connection_id));
        Ok(result)
    }

    pub async fn send_payload(&self, payload: &[u8]) -> bool {
        if !self.inner.send_payload(payload).await {
            self.inner.disconnect().await;
            return false;
        }
        true
    }
}

async fn read_loop(
    mut read: OwnedReadHalf,
    gateway_connection: Arc<TcpGatewayConnection>,
    write: Arc<TcpConnectionInner>,
    connection_id: u32,
) {
    let mut buf = crate::tcp_utils::allocated_read_buffer();

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
                crate::tcp_gateway::scripts::send_connection_error(
                    gateway_connection.as_ref(),
                    connection_id,
                    err.as_str(),
                    true,
                )
                .await;
                break;
            }
        };

        if read_size == 0 {
            let err = format!(
                "ReadLoop. Connection {connection_id} using gateway {} id disconnected",
                gateway_connection.gateway_id.as_str(),
            );

            crate::tcp_gateway::scripts::send_connection_error(
                gateway_connection.as_ref(),
                connection_id,
                err.as_str(),
                false,
            )
            .await;
            break;
        }

        let buffer = &buf[..read_size];

        gateway_connection
            .send_backward_payload(connection_id, buffer)
            .await;
    }
}
