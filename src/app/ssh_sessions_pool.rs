use std::{collections::HashMap, sync::Arc};

use my_ssh::{SshCredentials, SshSession};
use tokio::sync::Mutex;

pub struct SshSessionUsage {
    instance: Arc<SshSession>,
    usage: isize,
}

impl SshSessionUsage {
    pub fn new(instance: Arc<SshSession>) -> Self {
        Self { instance, usage: 1 }
    }
}

pub struct SshSessionsPool {
    data: Mutex<HashMap<String, SshSessionUsage>>,
}

impl SshSessionsPool {
    pub fn new() -> Self {
        Self {
            data: Default::default(),
        }
    }
    pub async fn get(&self, credentials: &Arc<SshCredentials>) -> Arc<SshSession> {
        let as_string = credentials.to_string();
        let mut write_access = self.data.lock().await;
        if let Some(result) = write_access.get_mut(as_string.as_str()) {
            result.usage += 1;
            return result.instance.clone();
        }

        let ssh_session = Arc::new(SshSession::new(credentials.clone()));
        write_access.insert(
            as_string.to_string(),
            SshSessionUsage::new(ssh_session.clone()),
        );
        ssh_session
    }

    pub async fn connection_is_dropped(&self, credentials: &Arc<SshCredentials>) {
        let as_string = credentials.to_string();

        let mut write_access = self.data.lock().await;

        let mut delete = false;
        if let Some(item) = write_access.get_mut(as_string.as_str()) {
            item.usage -= 1;

            if item.usage == 0 {
                delete = true;
            }
        }

        if delete {
            write_access.remove(as_string.as_str());
            println!("Ssh session `{}` is dropped", as_string.as_str());
        }
    }
}
