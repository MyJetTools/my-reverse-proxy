use std::sync::Arc;

use my_ssh::{SshCredentials, SshPortForwardTunnel};

pub struct SshToHttpPortForwardConfiguration {
    pub listen_port: u16,
    pub ssh_credentials: Arc<SshCredentials>,
    pub tunnel: Arc<SshPortForwardTunnel>,
}
