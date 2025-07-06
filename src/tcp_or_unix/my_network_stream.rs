use tokio::io::AsyncWriteExt;

use super::*;

pub enum MyNetworkStream {
    Tcp(tokio::net::TcpStream),
    #[cfg(unix)]
    UnixSocket(tokio::net::UnixStream),
    #[cfg(unix)]
    Ssh(my_ssh::SshAsyncChannel),
}

impl MyNetworkStream {
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
