use std::net::SocketAddr;

use tokio::io::AsyncWriteExt;

use crate::types::ConnectionIp;

pub enum AcceptedServerConnection {
    Tcp {
        network_stream: tokio::net::TcpStream,
        addr: SocketAddr,
    },

    Unix(tokio::net::UnixStream),
}

impl Into<AcceptedServerConnection> for (tokio::net::TcpStream, SocketAddr) {
    fn into(self) -> AcceptedServerConnection {
        AcceptedServerConnection::Tcp {
            network_stream: self.0,
            addr: self.1,
        }
    }
}

impl Into<AcceptedServerConnection> for tokio::net::UnixStream {
    fn into(self) -> AcceptedServerConnection {
        AcceptedServerConnection::Unix(self)
    }
}

impl AcceptedServerConnection {
    pub async fn shutdown(&mut self) {
        match self {
            AcceptedServerConnection::Tcp {
                network_stream,
                addr: _,
            } => {
                let _ = network_stream.shutdown().await;
            }
            AcceptedServerConnection::Unix(network_stream) => {
                let _ = network_stream.shutdown().await;
            }
        }
    }

    pub fn get_addr(&self) -> ConnectionIp {
        match self {
            AcceptedServerConnection::Tcp { addr, .. } => ConnectionIp::Tcp(*addr),
            AcceptedServerConnection::Unix(_) => ConnectionIp::UnixSocket,
        }
    }
}
