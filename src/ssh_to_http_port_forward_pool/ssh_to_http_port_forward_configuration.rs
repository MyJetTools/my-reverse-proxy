use std::sync::Arc;

use my_ssh::{SshCredentials, SshPortForwardTunnel};

pub struct SshToHttpPortForwardConfiguration {
    pub listen_port: u16,
    pub ssh_credentials: Arc<SshCredentials>,
    pub tunnel: Arc<SshPortForwardTunnel>,
}

impl SshToHttpPortForwardConfiguration {
    pub fn get_unix_socket_path(&self) -> String {
        format!("/var/my-reverse-proxy-{}", self.listen_port)
    }
}
