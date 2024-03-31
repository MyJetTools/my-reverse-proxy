use std::sync::Arc;

use my_ssh::SshCredentials;

use crate::{http_proxy_pass::HttpServerConnectionInfo, types::WhiteListedIpList};

use super::RemoteHost;

pub enum EndpointType {
    Http1(HttpServerConnectionInfo),
    Http2(HttpServerConnectionInfo),
    Https(super::SslCertificateId),

    Tcp {
        remote_addr: std::net::SocketAddr,
        debug: bool,
        whitelisted_ip: WhiteListedIpList,
    },
    TcpOverSsh {
        ssh_credentials: Arc<SshCredentials>,
        remote_host: RemoteHost,
        debug: bool,
    },
}
