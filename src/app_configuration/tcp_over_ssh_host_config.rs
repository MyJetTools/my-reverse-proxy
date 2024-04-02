use std::sync::Arc;

use my_ssh::SshCredentials;

use crate::settings::{HostString, RemoteHost};

pub struct TcpOverSshEndpointHostConfig {
    pub host: HostString,
    pub ssh_credentials: Arc<SshCredentials>,
    pub remote_host: Arc<RemoteHost>,
    pub debug: bool,
}
