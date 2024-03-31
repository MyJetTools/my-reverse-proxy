use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::http_server::ClientCertificateCa;

pub struct ClientCertificatesCache {
    data: Mutex<HashMap<String, Arc<ClientCertificateCa>>>,
}

impl ClientCertificatesCache {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }

    pub async fn insert_if_not_exists(&self, cert_id: &str, value: Arc<ClientCertificateCa>) {
        let mut write_access = self.data.lock().await;
        if write_access.contains_key(cert_id) {
            return;
        }
        write_access.insert(cert_id.to_string(), value);
    }

    pub async fn get(&self, cert_id: &str) -> Option<Arc<ClientCertificateCa>> {
        let read_access = self.data.lock().await;
        return read_access.get(cert_id).cloned();
    }
}
