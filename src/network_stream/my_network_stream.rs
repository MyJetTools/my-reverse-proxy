use std::{sync::Arc, time::Duration};

use my_ssh::SshSession;
use rust_extensions::remote_endpoint::RemoteEndpointOwned;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream;

use super::*;

pub enum MyNetworkStream {
    Tcp(tokio::net::TcpStream),
    #[cfg(unix)]
    UnixSocket(tokio::net::UnixStream),
}

impl MyNetworkStream {
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        match self {
            MyNetworkStream::Tcp(tcp_stream) => tcp_stream
                .read(buf)
                .await
                .map_err(|err| format!("{:?}", err)),
            MyNetworkStream::UnixSocket(unix_stream) => unix_stream
                .read(buf)
                .await
                .map_err(|err| format!("{:?}", err)),
        }
    }

    pub async fn shutdown(&mut self) {
        match self {
            MyNetworkStream::Tcp(tcp_stream) => {
                let _ = tcp_stream.shutdown().await;
            }
            MyNetworkStream::UnixSocket(unix_socket) => {
                let _ = unix_socket.shutdown().await;
            }
        }
    }

    pub fn into_split(self) -> (MyOwnedReadHalf, MyOwnedWriteHalf) {
        match self {
            MyNetworkStream::Tcp(tcp_stream) => {
                let result = tcp_stream.into_split();
                (result.0.into(), result.1.into())
            }
            MyNetworkStream::UnixSocket(unix_stream) => {
                let result = unix_stream.into_split();
                (result.0.into(), result.1.into())
            }
        }
    }
}

impl Into<MyNetworkStream> for tokio::net::TcpStream {
    fn into(self) -> MyNetworkStream {
        MyNetworkStream::Tcp(self)
    }
}

impl Into<MyNetworkStream> for tokio::net::UnixStream {
    fn into(self) -> MyNetworkStream {
        MyNetworkStream::UnixSocket(self)
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart for tokio::net::TcpStream {
    async fn shutdown_socket(&mut self) {
        let _ = self.shutdown().await;
    }

    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.write_all(buffer).await
    }

    async fn flush_it(&mut self) -> Result<(), NetworkError> {
        self.flush().await?;
        Ok(())
    }
}

pub trait NetworkStream {
    type WritePart: NetworkStreamWritePart + Send + Sync + 'static;
    type ReadPart: NetworkStreamReadPart + Send + Sync + 'static;
    fn split(self) -> (Self::ReadPart, Self::WritePart);

    async fn connect(
        gateway_id: Option<&Arc<String>>,
        ssh_session: Option<Arc<SshSession>>,
        server_name: Option<&str>,
        remote_endpoint: &Arc<RemoteEndpointOwned>,
        timeout: Duration,
    ) -> Result<Self, NetworkError>
    where
        Self: Sized;
}

impl NetworkStream for my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream> {
    type WritePart =
        tokio::io::WriteHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>;
    type ReadPart =
        tokio::io::ReadHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>;

    fn split(self) -> (Self::ReadPart, Self::WritePart) {
        tokio::io::split(self)
    }

    async fn connect(
        _gateway_id: Option<&Arc<String>>,
        _ssh_session: Option<Arc<SshSession>>,
        server_name: Option<&str>,
        remote_endpoint: &Arc<RemoteEndpointOwned>,
        timeout: Duration,
    ) -> Result<Self, NetworkError> {
        let connect =
            crate::http_client_connectors::connect_tls(remote_endpoint, server_name, false);

        let Ok(result) = tokio::time::timeout(timeout, connect).await else {
            return Err(NetworkError::Timeout(timeout));
        };

        Ok(result?)
    }
}

impl NetworkStream for my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream> {
    type WritePart =
        tokio::io::WriteHalf<my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>>;
    type ReadPart =
        tokio::io::ReadHalf<my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>>;

    fn split(self) -> (Self::ReadPart, Self::WritePart) {
        tokio::io::split(self)
    }

    async fn connect(
        _gateway_id: Option<&Arc<String>>,
        _ssh_session: Option<Arc<SshSession>>,
        _server_name: Option<&str>,
        _remote_endpoint: &Arc<RemoteEndpointOwned>,
        _timeout: Duration,
    ) -> Result<Self, NetworkError> {
        panic!("Not supported");
    }
}

impl NetworkStream for tokio::net::TcpStream {
    type WritePart = tokio::io::WriteHalf<tokio::net::TcpStream>;
    type ReadPart = tokio::io::ReadHalf<tokio::net::TcpStream>;

    fn split(self) -> (Self::ReadPart, Self::WritePart) {
        tokio::io::split(self)
    }

    async fn connect(
        _gateway_id: Option<&Arc<String>>,
        _ssh_session: Option<Arc<SshSession>>,
        _server_name: Option<&str>,
        remote_endpoint: &Arc<RemoteEndpointOwned>,
        timeout: Duration,
    ) -> Result<Self, NetworkError> {
        let host_port = remote_endpoint.get_host_port();
        let connect = tokio::net::TcpStream::connect(host_port.as_str());
        let Ok(result) = tokio::time::timeout(timeout, connect).await else {
            return Err(NetworkError::Timeout(timeout));
        };

        Ok(result?)
    }
}

#[cfg(unix)]
impl NetworkStream for tokio::net::UnixStream {
    type WritePart = tokio::io::WriteHalf<tokio::net::UnixStream>;
    type ReadPart = tokio::io::ReadHalf<tokio::net::UnixStream>;

    fn split(self) -> (Self::ReadPart, Self::WritePart) {
        tokio::io::split(self)
    }

    async fn connect(
        _gateway_id: Option<&Arc<String>>,
        _ssh_session: Option<Arc<SshSession>>,
        _server_name: Option<&str>,
        remote_endpoint: &Arc<RemoteEndpointOwned>,
        timeout: Duration,
    ) -> Result<Self, NetworkError> {
        let host_port = remote_endpoint.get_host_port();
        let connect = tokio::net::UnixStream::connect(host_port.as_str());
        let Ok(result) = tokio::time::timeout(timeout, connect).await else {
            return Err(NetworkError::Timeout(timeout));
        };

        Ok(result?)
    }
}

#[cfg(unix)]
impl NetworkStream for my_ssh::SshAsyncChannel {
    type WritePart = tokio::io::WriteHalf<my_ssh::SshAsyncChannel>;
    type ReadPart = tokio::io::ReadHalf<my_ssh::SshAsyncChannel>;

    fn split(self) -> (Self::ReadPart, Self::WritePart) {
        tokio::io::split(self)
    }

    async fn connect(
        _gateway_id: Option<&Arc<String>>,
        ssh_session: Option<Arc<SshSession>>,
        _server_name: Option<&str>,
        remote_endpoint: &Arc<RemoteEndpointOwned>,
        timeout: Duration,
    ) -> Result<Self, NetworkError> {
        let result = ssh_session
            .unwrap()
            .connect_to_remote_host(
                remote_endpoint.get_host(),
                remote_endpoint.get_port().unwrap(),
                timeout,
            )
            .await;

        match result {
            Ok(result) => Ok(result),
            Err(err) => match err {
                my_ssh::SshSessionError::Timeout => Err(NetworkError::Timeout(timeout)),
                _ => Err(NetworkError::Other(format!("{:?}", err))),
            },
        }
    }
}

impl NetworkStream for TcpGatewayProxyForwardStream {
    type WritePart = TcpGatewayProxyForwardStream;
    type ReadPart = TcpGatewayProxyForwardStream;

    fn split(self) -> (Self::ReadPart, Self::WritePart) {
        let write_part = self.clone();
        (self, write_part)
    }

    async fn connect(
        gateway_id: Option<&Arc<String>>,
        _ssh_session: Option<Arc<SshSession>>,
        _server_name: Option<&str>,
        remote_endpoint: &Arc<RemoteEndpointOwned>,
        timeout: Duration,
    ) -> Result<Self, NetworkError> {
        let gateway_id = gateway_id.unwrap();
        let Some(connection) = crate::app::APP_CTX
            .get_gateway_by_id_with_next_connection_id(&gateway_id)
            .await
        else {
            return Err(NetworkError::Other(format!(
                "Gateway with ID '{}' is not found",
                gateway_id
            )));
        };

        let (connection, id) = connection;

        match connection
            .connect_to_forward_proxy_connection(remote_endpoint.clone(), timeout, id)
            .await
        {
            Ok(result) => {
                println!("Connected to gateway");
                Ok(result)
            }
            Err(err) => Err(NetworkError::Other(format!("{:?}", err))),
        }
    }
}
