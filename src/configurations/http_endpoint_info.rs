use std::{net::SocketAddr, sync::Arc};

use crate::{
    http_proxy_pass::AllowedUserList,
    settings::{GoogleAuthSettings, HttpEndpointModifyHeadersSettings},
};

use super::*;

pub struct HttpEndpointInfo {
    pub host_endpoint: EndpointHttpHostString,
    pub debug: bool,
    pub http_type: HttpType,
    pub g_auth: Option<GoogleAuthSettings>,
    pub ssl_certificate_id: Option<SslCertificateId>,
    pub client_certificate_id: Option<SslCertificateId>,
    pub locations: Vec<Arc<ProxyPassLocationConfig>>,
    pub allowed_user_list: Option<Arc<AllowedUserList>>,
    pub modify_headers_settings: HttpEndpointModifyHeadersSettings,
}

impl HttpEndpointInfo {
    pub fn new(
        host_endpoint: EndpointHttpHostString,
        http_type: HttpType,
        debug: bool,
        g_auth: Option<GoogleAuthSettings>,
        ssl_certificate_id: Option<SslCertificateId>,
        client_certificate_id: Option<SslCertificateId>,
        locations: Vec<Arc<ProxyPassLocationConfig>>,
        allowed_user_list: Option<Arc<AllowedUserList>>,
        modify_headers_settings: HttpEndpointModifyHeadersSettings,
    ) -> Self {
        if debug {
            println!("Endpoint {} is in debug mode", host_endpoint.as_str());
        }
        Self {
            host_endpoint,
            debug,
            http_type,
            g_auth,
            client_certificate_id,
            locations,
            allowed_user_list,
            modify_headers_settings,
            ssl_certificate_id,
        }
    }

    pub fn is_my_endpoint(&self, server_name: &str) -> bool {
        self.host_endpoint.is_my_server_name(server_name)
    }

    pub fn as_str(&self) -> &str {
        self.host_endpoint.as_str()
    }

    pub fn get_listening_port_info(&self, socket_addr: SocketAddr) -> HttpListenPortInfo {
        HttpListenPortInfo {
            http_type: self.http_type,
            socket_addr,
        }
    }
}
