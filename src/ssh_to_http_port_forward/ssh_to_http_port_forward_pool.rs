use std::sync::Arc;

use my_ssh::{SshCredentials, SshSession};
use tokio::sync::Mutex;

use crate::configurations::RemoteHost;

use super::SshToHttpPortForwardConfiguration;

pub struct SshToHttpPortForwardPool {
    items: Mutex<Vec<Arc<SshToHttpPortForwardConfiguration>>>,
}

impl SshToHttpPortForwardPool {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
        }
    }

    pub async fn get_or_create_port_forward(
        &self,
        ssh_credentials: &Arc<SshCredentials>,
        remote_host: &RemoteHost,
    ) -> Arc<SshToHttpPortForwardConfiguration> {
        let mut access = self.items.lock().await;

        for itm in access.iter() {
            if itm.ssh_credentials.are_same(ssh_credentials)
                && itm.tunnel.remote_host == remote_host.get_host()
                && itm.tunnel.remote_port == remote_host.get_port()
            {
                return itm.clone();
            }
        }

        let listen_host_port = super::generate_unix_socket(listen_port);

        let ssh_session = SshSession::new(ssh_credentials.clone());

        let result = ssh_session
            .start_port_forward(listen_host_port, remote_host.to_string(), remote_port)
            .await
            .unwrap();

        let configuration = SshToHttpPortForwardConfiguration {
            listen_port,
            ssh_credentials: ssh_credentials.clone(),
            _ssh_session: ssh_session,
            tunnel: result.clone(),
        };

        let configuration = Arc::new(configuration);

        access.push(configuration.clone());

        return configuration;
    }

    pub async fn clean_up(&self) {
        let mut access = self.items.lock().await;
        for itm in access.drain(..) {
            println!("Stopping port forward: {}", itm.tunnel.listen_string);
            itm.tunnel.stop().await;
        }
    }
}
