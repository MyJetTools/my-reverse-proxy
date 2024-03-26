use std::sync::Arc;

use my_ssh::SshCredentials;

use crate::http_proxy_pass::ProxyPassEndpointInfo;

use super::{RemoteHost, SslCertificateId};

#[derive(Debug)]
pub enum EndpointType {
    Http1(ProxyPassEndpointInfo),
    Https {
        endpoint_info: ProxyPassEndpointInfo,
        ssl_id: super::SslCertificateId,
        client_ca_id: Option<SslCertificateId>,
    },
    Https2 {
        endpoint_info: ProxyPassEndpointInfo,
        ssl_id: super::SslCertificateId,
        client_ca_id: Option<SslCertificateId>,
    },
    Http2(ProxyPassEndpointInfo),
    Tcp {
        remote_addr: std::net::SocketAddr,
        debug: bool,
    },
    TcpOverSsh {
        ssh_credentials: Arc<SshCredentials>,
        remote_host: RemoteHost,
        debug: bool,
    },
}
