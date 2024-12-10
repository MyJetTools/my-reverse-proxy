use std::{collections::HashMap, sync::Arc};

use super::*;

#[derive(Clone)]
pub enum ListenConfiguration {
    Http(Arc<HttpListenPortConfiguration>),
    Tcp(Arc<TcpEndpointHostConfig>),
}

impl ListenConfiguration {
    pub fn get_white_list_id(&self) -> Option<&str> {
        match self {
            ListenConfiguration::Http(_) => None,
            ListenConfiguration::Tcp(config) => config.ip_white_list_id.as_deref(),
        }
    }
}

pub struct AppConfigurationInner {
    pub listen_endpoints: HashMap<u16, ListenConfiguration>,
    pub google_auth_credentials: GoogleAuthCredentialsList,
    pub white_list_ip_list: WhiteListedIpListConfigurations,
}

impl AppConfigurationInner {
    pub fn new() -> Self {
        Self {
            listen_endpoints: HashMap::new(),
            google_auth_credentials: GoogleAuthCredentialsList::new(),
            white_list_ip_list: WhiteListedIpListConfigurations::new(),
        }
    }

    pub fn get_http_endpoint_info(
        &self,
        listen_port: u16,
        server_name: &str,
    ) -> Option<Arc<HttpEndpointInfo>> {
        let listen_configuration = self.listen_endpoints.get(&listen_port)?;

        match listen_configuration {
            ListenConfiguration::Http(http_port_configuration) => {
                for endpoint_info in &http_port_configuration.endpoints {
                    if endpoint_info.is_my_endpoint(server_name) {
                        return Some(endpoint_info.clone());
                    }
                }
            }
            ListenConfiguration::Tcp(_) => {
                panic!(
                    "Port:{}. ServerName: {}. Http Endpoint info is requested for Tcp endpoint",
                    listen_port, server_name
                );
            }
        }

        None
    }
}
