use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use super::*;

#[derive(Clone)]
pub enum ListenConfiguration {
    Http(Arc<HttpListenPortConfiguration>),
    Tcp(Arc<TcpEndpointHostConfig>),
    Mpc(Arc<McpEndpointHostConfig>),
}

impl ListenConfiguration {
    pub fn get_white_list_id(&self) -> Option<&str> {
        match self {
            ListenConfiguration::Http(_) => None,
            ListenConfiguration::Tcp(config) => config.ip_white_list_id.as_deref(),
            ListenConfiguration::Mpc(_) => None,
        }
    }
}

pub struct AppConfigurationInner {
    pub listen_endpoints: HashMap<u16, ListenConfiguration>,
    pub google_auth_credentials: GoogleAuthCredentialsList,
    pub white_list_ip_list: WhiteListedIpListConfigurations,
    pub error_configurations: BTreeMap<String, String>,
}

impl AppConfigurationInner {
    pub fn new() -> Self {
        Self {
            listen_endpoints: HashMap::new(),
            google_auth_credentials: GoogleAuthCredentialsList::new(),
            white_list_ip_list: WhiteListedIpListConfigurations::new(),
            error_configurations: BTreeMap::new(),
        }
    }
}
