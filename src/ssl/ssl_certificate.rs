use std::sync::Arc;

use rustls_pki_types::{CertificateDer, PrivateKeyDer};

use my_tls::tokio_rustls;

#[derive(Clone, Debug)]
pub struct SslCertificate {
    pub cert_key: Arc<tokio_rustls::rustls::sign::CertifiedKey>,
}

impl SslCertificate {
    pub fn new(certificates: Vec<u8>, private_key: Vec<u8>, private_key_file_name: &str) -> Self {
        let cert_key = calc_cert_key(
            &super::certificates::load_private_key(private_key.clone(), private_key_file_name),
            super::certificates::load_certs(certificates.clone()),
        );

        SslCertificate {
            cert_key: Arc::new(cert_key),
        }
    }

    pub fn get_certified_key(&self) -> Arc<tokio_rustls::rustls::sign::CertifiedKey> {
        self.cert_key.clone()
    }
}

pub fn calc_cert_key(
    private_key: &PrivateKeyDer<'static>,
    certificates: Vec<CertificateDer<'static>>,
) -> tokio_rustls::rustls::sign::CertifiedKey {
    let private_key =
        tokio_rustls::rustls::crypto::aws_lc_rs::sign::any_supported_type(private_key).unwrap();
    tokio_rustls::rustls::sign::CertifiedKey::new(certificates.clone(), private_key)
}
