use std::sync::Arc;

use arc_swap::ArcSwapOption;

use super::ClientCertificateData;

pub struct ClientCertCell {
    pub value: ArcSwapOption<ClientCertificateData>,
}

impl ClientCertCell {
    pub fn new() -> Self {
        Self {
            value: ArcSwapOption::empty(),
        }
    }

    pub fn set(&self, value: Arc<ClientCertificateData>) {
        self.value.store(Some(value));
    }

    pub fn get(&self) -> Option<Arc<ClientCertificateData>> {
        self.value.swap(None)
    }
}
