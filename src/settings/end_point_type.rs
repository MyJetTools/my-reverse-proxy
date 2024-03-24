use std::sync::Arc;

use my_ssh::SshCredentials;

use super::{RemoteHost, SslCertificateId};

#[derive(Debug)]
pub enum EndpointType {
    Http1 {
        host_str: String,
        debug: bool,
    },
    Https {
        host_str: String,
        ssl_id: super::SslCertificateId,
        client_ca_id: Option<SslCertificateId>,
        debug: bool,
    },
    Https2 {
        host_str: String,
        ssl_id: super::SslCertificateId,
        client_ca_id: Option<SslCertificateId>,
        debug: bool,
    },
    Http2 {
        host_str: String,
        debug: bool,
    },
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
