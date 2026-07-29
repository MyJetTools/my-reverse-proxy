use std::{collections::HashMap, sync::Arc};

use my_ssh::SshCredentials;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum SshConfig {
    PrivateKey {
        private_key: String,
        pass_phrase: Option<String>,
    },

    Password(String),
}

pub struct SshConfigListInternal {
    data: HashMap<String, Arc<SshConfig>>,
}

impl SshConfigListInternal {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

pub struct SshConfigList {
    inner: RwLock<SshConfigListInternal>,
}

impl SshConfigList {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(SshConfigListInternal::new()),
        }
    }

    pub async fn get(&self, ssh_credentials: &SshCredentials) -> Option<Arc<SshConfig>> {
        let inner = self.inner.read().await;

        let id = ssh_credentials.to_string();

        if let Some(result) = inner.data.get(id.as_str()).cloned() {
            return Some(result);
        }

        if id.ends_with(":22") {
            if let Some(result) = inner.data.get(&id[..id.len() - 3]).cloned() {
                return Some(result);
            }
        }

        None
    }

    pub async fn clear_and_init(&self, data: impl Iterator<Item = (String, SshConfig)>) {
        let mut inner = self.inner.write().await;
        inner.data.clear();

        for (id, config) in data {
            inner.data.insert(id, Arc::new(config));
        }
    }
}
