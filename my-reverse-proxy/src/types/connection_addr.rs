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

    /// ISO-3 country code for the source IP (flag file name in the UI), when
    /// resolvable.
    pub fn get_country_log(&self) -> Option<String> {
        crate::ip_db::lookup_country_iso3(self.get_ip_addr()?)
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
