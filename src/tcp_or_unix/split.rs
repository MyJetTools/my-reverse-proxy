use my_ssh::SshAsyncChannel;

pub enum MyOwnedReadHalf {
    Tcp(tokio::net::tcp::OwnedReadHalf),
    Unix(tokio::net::unix::OwnedReadHalf),
    Ssh(futures::io::ReadHalf<SshAsyncChannel>),
}

impl MyOwnedReadHalf {
    pub async fn read(&mut self, buf: &mut Vec<u8>) -> Result<usize, std::io::Error> {
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

pub enum MyOwnedWriteHalf {
    Tcp(tokio::net::tcp::OwnedWriteHalf),
    Unix(tokio::net::unix::OwnedWriteHalf),
    Ssh(futures::io::WriteHalf<SshAsyncChannel>),
}

impl MyOwnedWriteHalf {
    pub async fn shutdown(&mut self) {
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

    pub async fn write_all(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
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
