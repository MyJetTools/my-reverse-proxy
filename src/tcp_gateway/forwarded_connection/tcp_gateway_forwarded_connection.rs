use std::{sync::Arc, time::Duration};

use encryption::aes::AesKey;

use crate::{network_stream::*, tcp_gateway::*};

pub struct TcpGatewayForwardConnection {
    inner: Arc<TcpConnectionInner>,
    connection_id: u32,
    receiver: Option<tokio::sync::mpsc::Receiver<()>>,
    read: Option<MyOwnedReadHalf>,
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

        let remote_endpoint_str = remote_endpoint.as_str();
        let tcp_stream = if remote_endpoint.starts_with('/') {
            connect_to_unix_socket(remote_endpoint_str, connection_id, timeout).await?
        } else {
            connect_to_tcp_socket(remote_endpoint_str, connection_id, timeout).await?
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
                "Connection: {}. Send to Forward Connection {}",
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
    mut read: MyOwnedReadHalf,
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

async fn connect_to_tcp_socket(
    remote_endpoint: &str,
    connection_id: u32,
    timeout: Duration,
) -> Result<MyNetworkStream, String> {
    let connect_future = tokio::net::TcpStream::connect(remote_endpoint);

    let result = tokio::time::timeout(timeout, connect_future).await;

    let Ok(result) = result else {
        return Err(format!(
            "Can not connect to tcp-socket {} with id {}. Err: Timeout {:?}",
            remote_endpoint, connection_id, timeout
        ));
    };

    match result {
        Ok(tcp_stream) => Ok(MyNetworkStream::Tcp(tcp_stream)),
        Err(err) => {
            return Err(format!(
                "Can not connect to tcp-socket {} with id {}. Err: {:?}",
                remote_endpoint, connection_id, err
            ));
        }
    }
}

async fn connect_to_unix_socket(
    remote_endpoint: &str,
    connection_id: u32,
    timeout: Duration,
) -> Result<MyNetworkStream, String> {
    let connect_future = tokio::net::UnixStream::connect(remote_endpoint);

    let result = tokio::time::timeout(timeout, connect_future).await;

    let Ok(result) = result else {
        return Err(format!(
            "Can not connect to unix socket {} with id {}. Err: Timeout {:?}",
            remote_endpoint, connection_id, timeout
        ));
    };

    match result {
        Ok(tcp_stream) => Ok(MyNetworkStream::UnixSocket(tcp_stream)),
        Err(err) => {
            return Err(format!(
                "Can not connect to unix socket {} with id {}. Err: {:?}",
                remote_endpoint, connection_id, err
            ));
        }
    }
}
