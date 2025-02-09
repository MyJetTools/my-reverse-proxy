use std::sync::Arc;

use super::*;

pub struct TcpEndpointHostConfig {
    pub host_endpoint: EndpointHttpHostString,
    pub remote_host: Arc<MyReverseProxyRemoteEndpoint>,
    pub debug: bool,
    pub ip_white_list_id: Option<String>,
}
