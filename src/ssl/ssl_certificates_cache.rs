use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use super::SslCertificate;

use crate::configurations::*;

use my_tls::tokio_rustls;
use tokio::sync::Mutex;

pub struct SslCertificateHolder {
    pub ssl_cert: SslCertificate,
    pub private_key_src: FileSource,
    pub cert_src: FileSource,
}

pub struct SslCertificatesCache {
    data: Mutex<HashMap<String, Arc<SslCertificateHolder>>>,
}

impl SslCertificatesCache {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_or_update(
        &self,
        cert_id: &SslCertificateId,
        ssl_cert: SslCertificate,
        private_key_src: FileSource,
        cert_src: FileSource,
    ) {
        let mut data = self.data.lock().await;
        data.insert(
            cert_id.to_string(),
            SslCertificateHolder {
                ssl_cert,
                private_key_src,
                cert_src,
            }
            .into(),
        );
    }

    pub async fn has_certificate(&self, cert_id: &SslCertificateId) -> bool {
        let data = self.data.lock().await;
        data.contains_key(cert_id.as_str())
    }

    pub async fn get_certified_key(
        &self,
        cert_id: &SslCertificateId,
    ) -> Option<Arc<tokio_rustls::rustls::sign::CertifiedKey>> {
        let data = self.data.lock().await;
        data.get(cert_id.as_str())
            .map(|holder| holder.ssl_cert.get_certified_key())
    }

    pub async fn get(&self, cert_id: &str) -> Option<Arc<SslCertificateHolder>> {
        let data = self.data.lock().await;
        data.get(cert_id).map(|holder| holder.clone())
    }

    pub async fn get_list(&self) -> BTreeMap<String, Arc<SslCertificateHolder>> {
        let mut result = BTreeMap::new();

        let data = self.data.lock().await;
        for itm in data.iter() {
            result.insert(itm.0.clone(), itm.1.clone());
        }
        result
    }
    /*
    pub fn get_ssl_key(&self, cert_id: &SslCertificateId) -> Option<Arc<SslCertificate>> {
        self.data
            .get(cert_id.as_str())
            .map(|ssl_cert| ssl_cert.clone())
    }
     */
}
