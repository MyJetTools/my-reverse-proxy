use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use super::*;

pub struct TcpEndpointHostConfig {
    pub host_endpoint: EndpointHttpHostString,
    pub remote_host: Arc<OverSshConnectionSettings>,
    pub debug: bool,
    pub ip_white_list_id: Option<String>,
}
