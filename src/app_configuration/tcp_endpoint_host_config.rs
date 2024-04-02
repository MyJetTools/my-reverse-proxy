use crate::{settings::HostString, types::WhiteListedIpList};

pub struct TcpEndpointHostConfig {
    pub host: HostString,
    pub remote_addr: std::net::SocketAddr,
    pub debug: bool,
    pub whitelisted_ip: WhiteListedIpList,
}
