use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

pub enum MyNetworkStream {
    Tcp(tokio::net::TcpStream),
    #[cfg(unix)]
    UnixSocket(tokio::net::UnixStream),
    #[cfg(unix)]
    Ssh(my_ssh::SshAsyncChannel),
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
            MyNetworkStream::Ssh(async_channel) => async_channel
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
            MyNetworkStream::Ssh(ssh) => {
                let _ = ssh.close().await;
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

            MyNetworkStream::Ssh(ssh) => {
                let result = futures::AsyncReadExt::split(ssh);
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

impl Into<MyNetworkStream> for my_ssh::SshAsyncChannel {
    fn into(self) -> MyNetworkStream {
        MyNetworkStream::Ssh(self)
    }
}

impl NetworkStreamWritePart for tokio::net::TcpStream {
    async fn shutdown_socket(&mut self) {
        let _ = self.shutdown().await;
    }

    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        self.write_all(buffer).await
    }
}
