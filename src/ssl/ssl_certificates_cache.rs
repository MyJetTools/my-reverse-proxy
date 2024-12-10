use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use my_ssh::ssh_settings::OverSshConnectionSettings;

use super::SslCertificate;

use crate::configurations::*;

pub struct SslCertificateHolder {
    pub ssl_cert: SslCertificate,
    pub private_key_src: OverSshConnectionSettings,
    pub cert_src: OverSshConnectionSettings,
}

pub struct SslCertificatesCache {
    data: HashMap<String, Arc<SslCertificateHolder>>,
}

impl SslCertificatesCache {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn add_or_update(
        &mut self,
        cert_id: SslCertificateIdRef,
        ssl_cert: SslCertificate,
        private_key_src: OverSshConnectionSettings,
        cert_src: OverSshConnectionSettings,
    ) {
        self.data.insert(
            cert_id.to_string(),
            SslCertificateHolder {
                ssl_cert,
                private_key_src,
                cert_src,
            }
            .into(),
        );
    }

    pub fn has_certificate(&self, cert_id: SslCertificateIdRef) -> bool {
        self.data.contains_key(cert_id.as_str())
    }

    pub fn get(&self, cert_id: SslCertificateIdRef) -> Option<Arc<SslCertificateHolder>> {
        self.data.get(cert_id.as_str()).map(|holder| holder.clone())
    }

    pub fn get_list(&self) -> BTreeMap<String, Arc<SslCertificateHolder>> {
        let mut result = BTreeMap::new();

        for itm in self.data.iter() {
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
