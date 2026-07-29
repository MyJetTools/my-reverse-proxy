use std::sync::Arc;

use crate::types::ListenHost;

use super::*;

#[derive(Clone)]
pub struct HttpListenPortConfiguration {
    pub endpoints: Vec<Arc<HttpEndpointInfo>>,
    pub listen_endpoint_type: ListenHttpEndpointType,
    pub listen_host: ListenHost,
}

impl HttpListenPortConfiguration {
    pub fn new(endpoint_info: Arc<HttpEndpointInfo>, listen_host: ListenHost) -> Self {
        let result = Self {
            listen_endpoint_type: endpoint_info.listen_endpoint_type,
            endpoints: vec![endpoint_info],
            listen_host,
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
            listen_host: endpoint_host_string.get_listen_host(),
        };

        Some(result)
    }

    pub fn get_http_endpoint_info(
        &self,
        server_name: Option<&str>,
    ) -> Option<Arc<HttpEndpointInfo>> {
        let Some(server_name) = server_name else {
            if self.endpoints.len() > 1 {
                return None;
            }

            let first = self.endpoints.first().unwrap();

            if !first.host_endpoint.has_server_name() {
                return Some(first.clone());
            }

            return None;
        };

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
