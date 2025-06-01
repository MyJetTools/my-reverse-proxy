use std::{sync::Arc, time::Duration};

use encryption::aes::AesKey;
use tokio::{
    io::AsyncReadExt,
    net::{tcp::OwnedReadHalf, TcpStream},
};

use crate::tcp_gateway::*;

pub struct TcpGatewayForwardConnection {
    inner: Arc<TcpConnectionInner>,
    connection_id: u32,
    receiver: Option<tokio::sync::mpsc::Receiver<()>>,
    read: Option<OwnedReadHalf>,
    gateway_connection: Arc<TcpGatewayConnection>,
    remote_endpoint: Arc<String>,
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
            "Gateway:[{}]. Establishing Forwarded connection to endpoint {} with id {}",
            gateway_connection.get_gateway_id().await,
            remote_endpoint.as_str(),
            connection_id
        );
        let connect_future = TcpStream::connect(remote_endpoint.as_str());

        let result = tokio::time::timeout(timeout, connect_future).await;

        if result.is_err() {
            return Err(format!(
                "Can not connect to {} with id {}. Err: Timeout {:?}",
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
                    "Can not connect to {} with id {}. Err: {:?}",
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
            connection_id,
            inner: inner.clone(),
            receiver: Some(receiver),
            read: Some(read),
            gateway_connection,
            remote_endpoint,
        };

        Ok(result)
    }

    pub async fn send_payload(&self, payload: &[u8]) -> bool {
        if !self.inner.send_payload(payload).await {
            println!(
                "Connection: {}. Send Forward {}",
                self.connection_id,
                payload.len()
            );
            self.inner.disconnect().await;
            return false;
        }
        true
    }

    pub async fn disconnect(&self) {
        self.inner.disconnect().await;
    }

    pub fn start(&mut self) {
        super::super::tcp_connection_inner::start_write_loop(
            self.inner.clone(),
            self.receiver.take().unwrap(),
        );

        let read = self.read.take().unwrap();

        tokio::spawn(read_loop(
            read,
            self.gateway_connection.clone(),
            self.inner.clone(),
            self.connection_id,
            self.remote_endpoint.clone(),
        ));
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
                    "Gateway:[{}]. ReadLoop. Can not read from connection {} with id {connection_id} created at: {} ConnectionError: {:?}",
                    gateway_connection.get_gateway_id().await,
                    remote_host.as_str(),
                    dt.to_rfc3339(),
                    err
                );
                crate::tcp_gateway::scripts::send_connection_error(
                    gateway_connection.as_ref(),
                    connection_id,
                    err.as_str(),
                    false,
                    true,
                )
                .await;
                break;
            }
        };

        if read_size == 0 {
            let err = format!(
                "ReadLoop. Connection to {} with id:{connection_id} is disconnected",
                remote_host.as_str(),
            );

            crate::tcp_gateway::scripts::send_connection_error(
                gateway_connection.as_ref(),
                connection_id,
                err.as_str(),
                false,
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
