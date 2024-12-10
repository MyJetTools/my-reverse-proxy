use std::collections::HashMap;

use my_ssh::SshCredentials;
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

    pub async fn get(&self, ssh_credentials: &SshCredentials) -> Option<String> {
        let id = ssh_credentials.to_string();
        let read_access = self.inner.lock().await;
        if let Some(pass_key) = read_access.pass_keys.get(&id) {
            return Some(pass_key.clone());
        }

        if id.ends_with(":22") {
            if let Some(pass_key) = read_access.pass_keys.get(&id[..&id.len() - 3]) {
                return Some(pass_key.clone());
            }
        }

        read_access.master_pass_key.clone()
    }
}
