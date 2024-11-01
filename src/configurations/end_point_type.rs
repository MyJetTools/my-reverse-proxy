use std::sync::Arc;

use super::*;

pub enum EndpointType {
    Http(HttpEndpointInfo),
    Tcp(Arc<TcpEndpointHostConfig>),
    TcpOverSsh(Arc<TcpOverSshEndpointHostConfig>),
}
