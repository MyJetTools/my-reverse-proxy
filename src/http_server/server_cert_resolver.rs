use std::sync::Arc;

use my_tls::tokio_rustls;

use my_tls::tokio_rustls::rustls::server::ResolvesServerCert;

#[derive(Debug)]
pub struct MyCertResolver {
    certified_key: Arc<tokio_rustls::rustls::sign::CertifiedKey>,
    debug: bool,
}

impl MyCertResolver {
    pub fn new(certified_key: Arc<tokio_rustls::rustls::sign::CertifiedKey>, debug: bool) -> Self {
        Self {
            certified_key,
            debug,
        }
    }
}

impl ResolvesServerCert for MyCertResolver {
    fn resolve(
        &self,
        client_hello: tokio_rustls::rustls::server::ClientHello,
    ) -> Option<std::sync::Arc<tokio_rustls::rustls::sign::CertifiedKey>> {
        if self.debug {
            println!("{:?}", client_hello.server_name());
        }

        Some(self.certified_key.clone())
    }
}
