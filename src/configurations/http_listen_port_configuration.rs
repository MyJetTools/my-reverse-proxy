use std::sync::Arc;

use crate::settings::SslCertificateId;

use super::*;

pub struct HttpListenPortConfiguration {
    pub endpoint_info: Vec<Arc<HttpEndpointInfo>>,
}

impl HttpListenPortConfiguration {
    pub fn new(endpoint_info: Arc<HttpEndpointInfo>) -> Self {
        let result = Self {
            endpoint_info: vec![endpoint_info],
        };

        result
    }

    pub fn is_http1(&self) -> bool {
        for info in &self.endpoint_info {
            if info.http_type.is_protocol_http1() {
                return true;
            }
        }
        false
    }

    pub fn is_https(&self) -> bool {
        for info in &self.endpoint_info {
            if info.http_type.is_https() {
                return true;
            }
        }
        false
    }

    pub fn get_ssl_certificate(&self, server_name: &str) -> Option<&SslCertificateId> {
        for endpoint_info in &self.endpoint_info {
            if endpoint_info.is_my_endpoint(server_name) {
                if let Some(ssl_id) = endpoint_info.ssl_certificate_id.as_ref() {
                    return Some(ssl_id);
                }
            }
        }

        None
    }

    pub fn get_ssl_certificates(&self) -> Option<Vec<&SslCertificateId>> {
        let mut result: Vec<&SslCertificateId> = vec![];

        for endpoint_info in &self.endpoint_info {
            if let Some(ssl_id) = endpoint_info.ssl_certificate_id.as_ref() {
                if !result.iter().any(|itm| itm.as_str() == ssl_id.as_str()) {
                    result.push(ssl_id);
                }
            }
        }

        if result.len() > 0 {
            return Some(result);
        }

        None
    }
}
