use std::sync::Arc;

use crate::settings::HttpEndpointModifyHeadersSettings;

use super::*;

pub struct HttpEndpointInfo {
    pub host_endpoint: EndpointHttpHostString,
    pub debug: bool,
    pub listen_endpoint_type: ListenHttpEndpointType,
    pub g_auth: Option<String>,
    pub ssl_certificate_id: Option<SslCertificateId>,
    pub client_certificate_id: Option<SslCertificateId>,
    pub locations: Vec<Arc<ProxyPassLocationConfig>>,
    pub allowed_user_list_id: Option<String>,
    pub modify_request_headers: ModifyHeadersConfig,
    pub modify_response_headers: ModifyHeadersConfig,
    pub whitelisted_ip_list_id: Option<String>,
}

impl HttpEndpointInfo {
    pub fn new(
        host_endpoint: EndpointHttpHostString,
        listen_endpoint_type: ListenHttpEndpointType,
        debug: bool,
        g_auth: Option<String>,
        ssl_certificate_id: Option<SslCertificateId>,
        client_certificate_id: Option<SslCertificateId>,
        whitelisted_ip_list_id: Option<String>,
        locations: Vec<Arc<ProxyPassLocationConfig>>,
        allowed_user_list_id: Option<String>,
        mut modify_headers_settings: HttpEndpointModifyHeadersSettings,
    ) -> Self {
        if debug {
            println!("Endpoint {} is in debug mode", host_endpoint.as_str());
        }
        Self {
            host_endpoint,
            debug,
            listen_endpoint_type,
            g_auth,
            client_certificate_id,
            locations,
            allowed_user_list_id,
            modify_request_headers: ModifyHeadersConfig::new_request(&mut modify_headers_settings),
            modify_response_headers: ModifyHeadersConfig::new_response(
                &mut modify_headers_settings,
            ),
            ssl_certificate_id,
            whitelisted_ip_list_id,
        }
    }

    pub fn is_my_endpoint(&self, server_name: &str) -> bool {
        self.host_endpoint.is_my_server_name(server_name)
    }

    pub fn as_str(&self) -> &str {
        self.host_endpoint.as_str()
    }

    pub fn find_location(&self, path: &str) -> Option<&ProxyPassLocationConfig> {
        for location in self.locations.iter() {
            if location.path.len() > path.len() {
                continue;
            }

            let path_prefix = &path[..location.path.len()];

            if path_prefix.eq_ignore_ascii_case(&location.path) {
                return Some(&location);
            }
        }

        None
    }
}
