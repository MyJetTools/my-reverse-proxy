use std::sync::Arc;

use rustls_pki_types::{CertificateDer, PrivateKeyDer};

#[derive(Clone, Debug)]
pub struct SslCertificate {
    pub certificates: Vec<CertificateDer<'static>>,
    pub private_key: Arc<PrivateKeyDer<'static>>,
}

impl SslCertificate {
    pub fn new(certificates: Vec<u8>, private_key: Vec<u8>, private_key_file_name: &str) -> Self {
        SslCertificate {
            certificates: super::certificates::load_certs(certificates),
            private_key: super::certificates::load_private_key(private_key, private_key_file_name)
                .into(),
        }
    }
}
