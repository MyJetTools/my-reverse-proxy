use std::time::Duration;

use my_ssh::SshAsyncChannel;
use tokio::io::AsyncReadExt;

#[async_trait::async_trait]
pub trait NetworkStreamReadPart {
    async fn read_from_socket(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error>;

    async fn read_with_timeout(
        &mut self,
        buf: &mut Vec<u8>,
        timeout: Duration,
    ) -> Result<usize, String> {
        let future = self.read_from_socket(buf);

        let result = tokio::time::timeout(timeout, future).await;

        let Ok(result) = result else {
            return Err("Read timeout".to_string());
        };

        result.map_err(|err| format!("{:?}", err))
    }
}

pub enum MyOwnedReadHalf {
    Tcp(tokio::net::tcp::OwnedReadHalf),
    Unix(tokio::net::unix::OwnedReadHalf),
    Ssh(futures::io::ReadHalf<SshAsyncChannel>),
}

impl MyOwnedReadHalf {
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        match self {
            MyOwnedReadHalf::Tcp(owned_read_half) => {
                use tokio::io::AsyncReadExt;
                owned_read_half.read(buf).await
            }
            MyOwnedReadHalf::Unix(owned_read_half) => {
                use tokio::io::AsyncReadExt;
                owned_read_half.read(buf).await
            }
            MyOwnedReadHalf::Ssh(owned_read_half) => {
                use futures::AsyncReadExt;
                owned_read_half.read(buf).await
            }
        }
    }
}

impl Into<MyOwnedReadHalf> for tokio::net::tcp::OwnedReadHalf {
    fn into(self) -> MyOwnedReadHalf {
        MyOwnedReadHalf::Tcp(self)
    }
}

impl Into<MyOwnedReadHalf> for tokio::net::unix::OwnedReadHalf {
    fn into(self) -> MyOwnedReadHalf {
        MyOwnedReadHalf::Unix(self)
    }
}

impl Into<MyOwnedReadHalf> for futures::io::ReadHalf<SshAsyncChannel> {
    fn into(self) -> MyOwnedReadHalf {
        MyOwnedReadHalf::Ssh(self)
    }
}

#[async_trait::async_trait]
impl NetworkStreamReadPart for MyOwnedReadHalf {
    async fn read_from_socket(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.read(buf).await
    }
}
#[async_trait::async_trait]
impl NetworkStreamReadPart
    for tokio::io::ReadHalf<my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>>
{
    async fn read_from_socket(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.read(buf).await
    }
}

#[async_trait::async_trait]
impl NetworkStreamReadPart for tokio::io::ReadHalf<tokio::net::TcpStream> {
    async fn read_from_socket(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.read(buf).await
    }
}
