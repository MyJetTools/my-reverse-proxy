use super::{SshConfiguration, SslCertificateId};

#[derive(Debug)]
pub enum EndpointType {
    Http1(String),
    Https {
        host_str: String,
        ssl_id: super::SslCertificateId,
        client_ca_id: Option<SslCertificateId>,
    },
    Http2(String),
    Tcp(std::net::SocketAddr),
    TcpOverSsh(SshConfiguration),
}
