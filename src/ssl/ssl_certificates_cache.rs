use std::{collections::HashMap, sync::Arc};

use super::SslCertificate;

use crate::configurations::*;

use my_tls::tokio_rustls;

pub struct SslCertificatesCache {
    data: HashMap<String, Arc<SslCertificate>>,
}

impl SslCertificatesCache {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn add(&mut self, cert_id: &SslCertificateId, ssl_cert: SslCertificate) {
        self.data.insert(cert_id.to_string(), ssl_cert.into());
    }

    pub fn has_certificate(&self, cert_id: &SslCertificateId) -> bool {
        self.data.contains_key(cert_id.as_str())
    }

    pub fn get_certified_key(
        &self,
        cert_id: &SslCertificateId,
    ) -> Option<Arc<tokio_rustls::rustls::sign::CertifiedKey>> {
        self.data
            .get(cert_id.as_str())
            .map(|ssl_cert| ssl_cert.get_certified_key())
    }

    /*
    pub fn get_ssl_key(&self, cert_id: &SslCertificateId) -> Option<Arc<SslCertificate>> {
        self.data
            .get(cert_id.as_str())
            .map(|ssl_cert| ssl_cert.clone())
    }
     */
}
