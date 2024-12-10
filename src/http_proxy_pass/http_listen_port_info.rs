use std::net::SocketAddr;

use crate::configurations::ListenHttpEndpointType;

#[derive(Clone)]
pub struct HttpListenPortInfo {
    pub endpoint_type: ListenHttpEndpointType,
    pub socket_addr: SocketAddr,
}
