use super::SshConfiguration;

#[derive(Debug)]
pub enum EndpointType {
    Http1,
    Https1(super::SslCertificateId),
    Http2,
    Tcp(std::net::SocketAddr),
    TcpOverSsh(SshConfiguration),
}
