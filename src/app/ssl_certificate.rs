#[derive(Clone)]
pub struct SslCertificate {
    pub certificates: Vec<rustls::Certificate>,
    pub private_key: rustls::PrivateKey,
}
