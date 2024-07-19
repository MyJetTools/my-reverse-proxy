use std::sync::Arc;

use my_ssh::{SshCredentials, SshSession};
use tokio::sync::Mutex;

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
        remote_host: &str,
        remote_port: u16,
        next_port: impl Fn() -> u16,
    ) -> Arc<SshToHttpPortForwardConfiguration> {
        let mut access = self.items.lock().await;

        for itm in access.iter() {
            if itm.ssh_credentials.are_same(ssh_credentials)
                && itm.tunnel.remote_host == remote_host
                && itm.tunnel.remote_port == remote_port
            {
                return itm.clone();
            }
        }

        let listen_port = next_port();

        println!(
            "Allocating listen port: {} for http port forward {}->{}:{}",
            listen_port,
            ssh_credentials.to_string(),
            remote_host,
            remote_port
        );

        let listen_host_port = format!("/var/my-reverse-proxy-{}", listen_port);

        let ssh_session = SshSession::new(ssh_credentials.clone());

        let result = ssh_session
            .start_port_forward(listen_host_port, remote_host.to_string(), remote_port)
            .await
            .unwrap();

        let configuration = SshToHttpPortForwardConfiguration {
            listen_port,
            ssh_credentials: ssh_credentials.clone(),
            tunnel: result.clone(),
        };

        let configuration = Arc::new(configuration);

        access.push(configuration.clone());

        return configuration;
    }
}
