use std::{collections::HashMap, sync::Arc};

use crate::http_server::ClientCertificateCa;

use crate::configurations::*;

pub struct ClientCertificatesCache {
    data: HashMap<String, Arc<ClientCertificateCa>>,
}

impl ClientCertificatesCache {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert(&mut self, cert_id: &SslCertificateId, value: Arc<ClientCertificateCa>) {
        self.data.insert(cert_id.to_string(), value);
    }

    pub fn has_certificate(&self, cert_id: &SslCertificateId) -> bool {
        return self.data.contains_key(cert_id.as_str());
    }

    pub fn get(&self, cert_id: &SslCertificateId) -> Option<Arc<ClientCertificateCa>> {
        return self.data.get(cert_id.as_str()).cloned();
    }
}
