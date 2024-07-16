use std::net::SocketAddr;

use super::*;

#[derive(Clone)]
pub struct HttpListenPortInfo {
    pub http_type: HttpType,
    pub socket_addr: SocketAddr,
}
