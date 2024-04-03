use crate::{settings::EndpointHttpHostString, types::WhiteListedIpList};

pub struct TcpEndpointHostConfig {
    pub host: EndpointHttpHostString,
    pub remote_addr: std::net::SocketAddr,
    pub debug: bool,
    pub whitelisted_ip: WhiteListedIpList,
}
