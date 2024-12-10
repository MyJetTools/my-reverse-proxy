use std::sync::{Arc, Mutex};

use super::ClientCertificateData;

pub struct ClientCertCell {
    pub value: Mutex<Option<Arc<ClientCertificateData>>>,
}

impl ClientCertCell {
    pub fn new() -> Self {
        Self {
            value: Mutex::new(None),
        }
    }

    pub fn set(&self, value: Arc<ClientCertificateData>) {
        let mut write_access = self.value.lock().unwrap();
        *write_access = Some(value);
    }

    pub fn get(&self) -> Option<Arc<ClientCertificateData>> {
        let mut read_access = self.value.lock().unwrap();
        return read_access.take();
    }
}
