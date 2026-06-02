use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Copy)]
pub enum ConnectionIp {
    Tcp(SocketAddr),
    UnixSocket,
}

impl ConnectionIp {
    pub fn get_ip_addr(&self) -> Option<IpAddr> {
        match self {
            ConnectionIp::Tcp(addr) => Some(addr.ip()),
            ConnectionIp::UnixSocket => None,
        }
    }

    /// Source IP as a string for the in-memory logs IP field; `None` for unix
    /// sockets (no IP to resolve).
    pub fn get_ip_log(&self) -> Option<String> {
        self.get_ip_addr().map(|ip| ip.to_string())
    }
}

impl Into<ConnectionIp> for SocketAddr {
    fn into(self) -> ConnectionIp {
        ConnectionIp::Tcp(self)
    }
}

impl std::fmt::Debug for ConnectionIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp(ip) => f.debug_tuple("Tcp:").field(ip).finish(),
            Self::UnixSocket => write!(f, "UnixSocket"),
        }
    }
}
