use std::sync::Arc;

use my_ssh::SshCredentials;

use super::{RemoteHost, SslCertificateId};

#[derive(Debug)]
pub enum EndpointType {
    Http1(String),
    Https {
        host_str: String,
        ssl_id: super::SslCertificateId,
        client_ca_id: Option<SslCertificateId>,
    },
    Https2 {
        host_str: String,
        ssl_id: super::SslCertificateId,
        client_ca_id: Option<SslCertificateId>,
    },
    Http2(String),
    Tcp(std::net::SocketAddr),
    TcpOverSsh {
        ssh_credentials: Arc<SshCredentials>,
        remote_host: RemoteHost,
    },
}
