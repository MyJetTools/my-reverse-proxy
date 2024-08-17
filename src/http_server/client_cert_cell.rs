use std::sync::Mutex;

use super::ClientCertificateData;

pub struct ClientCertCell {
    pub value: Mutex<Option<ClientCertificateData>>,
}

impl ClientCertCell {
    pub fn new() -> Self {
        Self {
            value: Mutex::new(None),
        }
    }

    pub fn set(&self, value: ClientCertificateData) {
        let mut write_access = self.value.lock().unwrap();
        *write_access = Some(value);
    }

    pub fn get(&self) -> Option<ClientCertificateData> {
        let mut read_access = self.value.lock().unwrap();
        return read_access.take();
    }
}
