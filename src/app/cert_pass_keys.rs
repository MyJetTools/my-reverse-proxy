use std::collections::HashMap;

use tokio::sync::Mutex;

#[derive(Debug, Default)]
pub struct CertPassKeysInner {
    pub master_pass_key: Option<String>,
    pub pass_keys: HashMap<String, String>,
}

pub struct CertPassKeys {
    pub inner: Mutex<CertPassKeysInner>,
}

impl CertPassKeys {
    pub fn new() -> Self {
        CertPassKeys {
            inner: Mutex::new(CertPassKeysInner::default()),
        }
    }

    pub async fn add(&self, key: String, pass_key: String) {
        let mut write_access = self.inner.lock().await;
        if key == "*" {
            write_access.master_pass_key = Some(pass_key);
        } else {
            write_access.pass_keys.insert(key, pass_key);
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let read_access = self.inner.lock().await;
        if let Some(pass_key) = read_access.pass_keys.get(key) {
            return Some(pass_key.clone());
        }

        read_access.master_pass_key.clone()
    }
}
