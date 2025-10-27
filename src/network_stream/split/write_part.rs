use std::time::Duration;

use my_ssh::SshAsyncChannel;
use tokio::io::AsyncWriteExt;

use crate::{
    network_stream::NetworkError, tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
};

#[async_trait::async_trait]
pub trait NetworkStreamWritePart {
    async fn shutdown_socket(&mut self);
    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error>;

    async fn write_all_with_timeout(
        &mut self,
        buffer: &[u8],
        timeout: Duration,
    ) -> Result<(), NetworkError> {
        let future = self.write_to_socket(buffer);

        let result = tokio::time::timeout(timeout, future).await;

        let Ok(result) = result else {
            self.shutdown_socket();
            return Err(NetworkError::Timeout(timeout));
        };

        if let Err(err) = result {
            self.shutdown_socket().await;
            return Err(NetworkError::IoError(err));
        }

        Ok(())
    }
}

pub enum MyOwnedWriteHalf {
    Tcp(tokio::net::tcp::OwnedWriteHalf),
    Unix(tokio::net::unix::OwnedWriteHalf),
    Ssh(futures::io::WriteHalf<SshAsyncChannel>),
}

impl Into<MyOwnedWriteHalf> for tokio::net::tcp::OwnedWriteHalf {
    fn into(self) -> MyOwnedWriteHalf {
        MyOwnedWriteHalf::Tcp(self)
    }
}

impl Into<MyOwnedWriteHalf> for tokio::net::unix::OwnedWriteHalf {
    fn into(self) -> MyOwnedWriteHalf {
        MyOwnedWriteHalf::Unix(self)
    }
}

impl Into<MyOwnedWriteHalf> for futures::io::WriteHalf<SshAsyncChannel> {
    fn into(self) -> MyOwnedWriteHalf {
        MyOwnedWriteHalf::Ssh(self)
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart for MyOwnedWriteHalf {
    async fn shutdown_socket(&mut self) {
        match self {
            MyOwnedWriteHalf::Tcp(owned_write_half) => {
                use tokio::io::AsyncWriteExt;
                let _ = owned_write_half.shutdown().await;
            }
            MyOwnedWriteHalf::Unix(owned_write_half) => {
                use tokio::io::AsyncWriteExt;
                let _ = owned_write_half.shutdown().await;
            }
            MyOwnedWriteHalf::Ssh(ssh) => {
                use futures::AsyncWriteExt;
                let _ = ssh.close().await;
            }
        }
    }

    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        match self {
            MyOwnedWriteHalf::Tcp(owned_write_half) => {
                use tokio::io::AsyncWriteExt;
                owned_write_half.write_all(buffer).await
            }
            MyOwnedWriteHalf::Unix(owned_write_half) => {
                use tokio::io::AsyncWriteExt;
                owned_write_half.write_all(buffer).await
            }
            MyOwnedWriteHalf::Ssh(owned_write_half) => {
                use futures::AsyncWriteExt;
                owned_write_half.write_all(buffer).await
            }
        }
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart
    for tokio::io::WriteHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>
{
    async fn shutdown_socket(&mut self) {
        let _ = self.shutdown().await;
    }

    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.write_all(buffer).await
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart
    for tokio::io::WriteHalf<my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>>
{
    async fn shutdown_socket(&mut self) {
        let _ = self.shutdown().await;
    }

    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.write_all(buffer).await
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart for tokio::io::WriteHalf<tokio::net::TcpStream> {
    async fn shutdown_socket(&mut self) {
        let _ = self.shutdown().await;
    }

    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.write_all(buffer).await
    }
}

#[cfg(unix)]
#[async_trait::async_trait]
impl NetworkStreamWritePart for tokio::io::WriteHalf<tokio::net::UnixStream> {
    async fn shutdown_socket(&mut self) {
        let _ = self.shutdown().await;
    }

    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.write_all(buffer).await
    }
}

#[cfg(unix)]
#[async_trait::async_trait]
impl NetworkStreamWritePart for tokio::io::WriteHalf<my_ssh::SshAsyncChannel> {
    async fn shutdown_socket(&mut self) {
        let _ = self.shutdown().await;
    }

    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.write_all(buffer).await
    }
}
#[async_trait::async_trait]
impl NetworkStreamWritePart for tokio::io::WriteHalf<TcpGatewayProxyForwardStream> {
    async fn shutdown_socket(&mut self) {
        let _ = self.shutdown().await;
    }
    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.write_all(buffer).await
    }
}
