use std::sync::Arc;

use super::*;

#[derive(Clone)]
pub struct HttpListenPortConfiguration {
    pub endpoints: Vec<Arc<HttpEndpointInfo>>,
    pub listen_endpoint_type: ListenHttpEndpointType,
}

impl HttpListenPortConfiguration {
    pub fn new(endpoint_info: Arc<HttpEndpointInfo>) -> Self {
        let result = Self {
            listen_endpoint_type: endpoint_info.listen_endpoint_type,
            endpoints: vec![endpoint_info],
        };

        result
    }

    pub fn insert_or_replace_configuration(&mut self, endpoint_info: HttpEndpointInfo) {
        let index = self
            .endpoints
            .iter()
            .position(|itm| itm.host_endpoint.as_str() == endpoint_info.as_str());

        match index {
            Some(index) => {
                self.endpoints[index] = Arc::new(endpoint_info);
            }
            None => {
                self.endpoints.push(Arc::new(endpoint_info));
            }
        }
    }

    pub fn delete_configuration(
        &self,
        endpoint_host_string: &EndpointHttpHostString,
    ) -> Option<Self> {
        let index = self
            .endpoints
            .iter()
            .position(|itm| itm.host_endpoint.as_str() == endpoint_host_string.as_str())?;

        let mut result = self.endpoints.clone();

        result.remove(index);

        let result = Self {
            listen_endpoint_type: self.listen_endpoint_type,
            endpoints: result,
        };

        Some(result)
    }

    pub fn get_http_endpoint_info(&self, server_name: &str) -> Option<Arc<HttpEndpointInfo>> {
        for endpoint_info in &self.endpoints {
            if endpoint_info.is_my_endpoint(server_name) {
                return Some(endpoint_info.clone());
            }
        }

        None
    }

    /*
       pub fn get_listen_endpoint_type(&self) -> ListenHttpEndpointType {
           self.endpoint_info[0].listen_endpoint_type
       }
    */
    pub fn get_ssl_certificate<'s>(
        &'s self,
        server_name: &str,
    ) -> Option<(SslCertificateIdRef<'s>, Arc<HttpEndpointInfo>)> {
        for endpoint_info in &self.endpoints {
            if endpoint_info.is_my_endpoint(server_name) {
                if let Some(ssl_id) = endpoint_info.ssl_certificate_id.as_ref() {
                    return Some((ssl_id.into(), endpoint_info.clone()));
                }
            }
        }

        None
    }
    /*
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
     */
}
