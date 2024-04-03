use std::sync::Arc;

use my_ssh::SshCredentials;

use crate::settings::{EndpointHttpHostString, RemoteHost};

pub struct TcpOverSshEndpointHostConfig {
    pub host: EndpointHttpHostString,
    pub ssh_credentials: Arc<SshCredentials>,
    pub remote_host: Arc<RemoteHost>,
    pub debug: bool,
}
