use std::sync::Arc;

use super::HttpEndpointInfo;

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
            if info.http_type.is_http1() {
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
}
