use super::{SshConfiguration, SslCertificateId};

#[derive(Debug)]
pub enum EndpointType {
    Http1,
    Https {
        ssl_id: super::SslCertificateId,
        client_ca_id: Option<SslCertificateId>,
    },
    Http2,
    Tcp(std::net::SocketAddr),
    TcpOverSsh(SshConfiguration),
}
