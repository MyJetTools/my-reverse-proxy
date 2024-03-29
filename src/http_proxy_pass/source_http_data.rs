use std::net::SocketAddr;

pub struct SourceHttpData {
    pub is_https: bool,
    pub socket_addr: SocketAddr,
}

impl SourceHttpData {
    pub fn new(socket_addr: SocketAddr) -> Self {
        Self {
            is_https: false,
            socket_addr,
        }
    }
}
