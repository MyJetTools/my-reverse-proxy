use std::net::SocketAddr;

use super::*;

#[derive(Clone)]
pub struct HttpListenPortInfo {
    pub port: u16,
    pub http_type: HttpType,
    pub socket_addr: SocketAddr,
}
