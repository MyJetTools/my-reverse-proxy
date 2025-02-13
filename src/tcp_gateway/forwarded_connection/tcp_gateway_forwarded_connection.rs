use std::{sync::Arc, time::Duration};

use encryption::aes::AesKey;
use tokio::{
    io::AsyncReadExt,
    net::{tcp::OwnedReadHalf, TcpStream},
};

use crate::tcp_gateway::*;

pub struct TcpGatewayForwardConnection {
    inner: Arc<TcpConnectionInner>,
}

impl TcpGatewayForwardConnection {
    pub async fn connect(
        connection_id: u32,
        gateway_connection: Arc<TcpGatewayConnection>,
        remote_endpoint: Arc<String>,
        timeout: Duration,
        aes_key: Arc<AesKey>,
    ) -> Result<Self, String> {
        println!(
            "Gateway [{}]. Establishing Forwarded connection to endpoint {} with id {}",
            gateway_connection.get_gateway_id().await,
            remote_endpoint.as_str(),
            connection_id
        );
        let connect_future = TcpStream::connect(remote_endpoint.as_str());

        let result = tokio::time::timeout(timeout, connect_future).await;

        if result.is_err() {
            return Err(format!(
                "Gateway [{}]. Can not connect to {} with id {}. Err: Timeout {:?}",
                gateway_connection.get_gateway_id().await,
                remote_endpoint.as_str(),
                connection_id,
                timeout
            ));
        }

        let result = result.unwrap();

        let tcp_stream = match result {
            Ok(tcp_stream) => tcp_stream,
            Err(err) => {
                return Err(format!(
                    "Gateway [{}]. Can not connect to {} with id {}. Err: {:?}",
                    gateway_connection.get_gateway_id().await,
                    remote_endpoint.as_str(),
                    connection_id,
                    err
                ));
            }
        };

        let (read, write) = tcp_stream.into_split();

        let (inner, receiver) = TcpConnectionInner::new(write, aes_key);

        let inner = Arc::new(inner);

        let result = Self {
            inner: inner.clone(),
        };

        super::super::tcp_connection_inner::start_write_loop(inner.clone(), receiver);

        tokio::spawn(read_loop(
            read,
            gateway_connection,
            inner,
            connection_id,
            remote_endpoint.clone(),
        ));
        Ok(result)
    }

    pub async fn send_payload(&self, payload: &[u8]) -> bool {
        if !self.inner.send_payload(payload).await {
            self.inner.disconnect().await;
            return false;
        }
        true
    }

    pub async fn disconnect(&self) {
        self.inner.disconnect().await;
    }
}

async fn read_loop(
    mut read: OwnedReadHalf,
    gateway_connection: Arc<TcpGatewayConnection>,
    write: Arc<TcpConnectionInner>,
    connection_id: u32,
    remote_host: Arc<String>,
) {
    let mut buf = crate::tcp_utils::allocated_read_buffer(None);

    loop {
        let read_size = match read.read(&mut buf).await {
            Ok(read_size) => read_size,
            Err(err) => {
                write.disconnect().await;

                let dt = gateway_connection.get_connection_timestamp();
                let err = format!(
                    "ReadLoop. Can not read from connection {} with id {connection_id} using gateway [{}] created at: {} ConnectionError: {:?}",
                    remote_host.as_str(),
                    gateway_connection.get_gateway_id().await,
                    dt.to_rfc3339(),
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
                "ReadLoop. Connection to {} with {connection_id} using gateway [{}] id disconnected",
                remote_host.as_str(),
                gateway_connection.get_gateway_id().await,
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
