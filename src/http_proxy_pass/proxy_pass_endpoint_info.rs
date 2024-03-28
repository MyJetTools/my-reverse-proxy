use crate::settings::GoogleAuthSettings;

use super::HttpType;

pub struct ProxyPassEndpointInfo {
    pub host_endpoint: String,
    pub debug: bool,
    pub http_type: HttpType,
    pub g_auth: Option<GoogleAuthSettings>,
}

impl ProxyPassEndpointInfo {
    pub fn new(
        host_endpoint: String,
        http_type: HttpType,
        debug: bool,
        g_auth: Option<GoogleAuthSettings>,
    ) -> Self {
        Self {
            host_endpoint,
            debug,
            http_type,
            g_auth,
        }
    }

    pub fn is_my_endpoint(&self, other_host_endpoint: &str) -> bool {
        self.host_endpoint == other_host_endpoint
    }

    pub fn as_str(&self) -> &str {
        &self.host_endpoint
    }
}
