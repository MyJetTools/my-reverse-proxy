use crate::settings::{GoogleAuthSettings, HostString, SslCertificateId};

use super::HttpType;

#[derive(Clone)]
pub struct HttpServerConnectionInfo {
    pub host_endpoint: HostString,
    pub debug: bool,
    pub http_type: HttpType,
    pub g_auth: Option<GoogleAuthSettings>,
    pub client_certificate_id: Option<SslCertificateId>,
}

impl HttpServerConnectionInfo {
    pub fn new(
        host_endpoint: HostString,
        http_type: HttpType,
        debug: bool,
        g_auth: Option<GoogleAuthSettings>,
        client_certificate_id: Option<SslCertificateId>,
    ) -> Self {
        Self {
            host_endpoint,
            debug,
            http_type,
            g_auth,
            client_certificate_id,
        }
    }

    pub fn is_my_endpoint(&self, other_host_endpoint: &str) -> bool {
        self.host_endpoint.eq(other_host_endpoint)
    }

    pub fn as_str(&self) -> &str {
        self.host_endpoint.as_str()
    }
}
