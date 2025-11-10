use std::sync::Arc;

use tokio::sync::RwLock;

use super::*;

pub struct AppConfiguration {
    inner: RwLock<AppConfigurationInner>,
}

impl AppConfiguration {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(AppConfigurationInner::new()),
        }
    }

    pub async fn get<TResult>(
        &self,
        func: impl FnOnce(&AppConfigurationInner) -> TResult,
    ) -> TResult {
        let inner = self.inner.read().await;
        func(&inner)
    }

    pub async fn write(&self, func: impl FnOnce(&mut AppConfigurationInner)) {
        let mut inner = self.inner.write().await;
        func(&mut inner)
    }

    pub async fn add_http_configuration_error(
        &self,
        host_id: &EndpointHttpHostString,
        error: String,
    ) {
        let endpoint_port = host_id.get_port();

        let mut inner = self.inner.write().await;

        match endpoint_port {
            EndpointPort::Tcp(port) => {
                if let Some(location) = inner.listen_tcp_endpoints.get_mut(&port) {
                    if let ListenConfiguration::Http(location) = location {
                        let location = location.delete_configuration(&host_id);

                        if let Some(location) = location {
                            if location.endpoints.len() == 0 {
                                inner.listen_tcp_endpoints.remove(&port);
                            } else {
                                inner
                                    .listen_tcp_endpoints
                                    .insert(port, ListenConfiguration::Http(Arc::new(location)));
                            }
                        }
                    }
                }
            }
            EndpointPort::UnixSocket(unix_host) => {
                if let Some(location) = inner.listen_unix_socket_endpoints.get_mut(&unix_host) {
                    if let ListenConfiguration::Http(location) = location {
                        let location = location.delete_configuration(&host_id);

                        if let Some(location) = location {
                            if location.endpoints.is_empty() {
                                inner.listen_unix_socket_endpoints.remove(&unix_host);
                            } else {
                                inner.listen_unix_socket_endpoints.insert(
                                    unix_host,
                                    ListenConfiguration::Http(Arc::new(location)),
                                );
                            }
                        }
                    }
                }
            }
        }

        inner
            .error_configurations
            .insert(host_id.as_str().to_string(), error);
    }
    /*
    pub async fn write_with_result<TResult>(
        &self,
        func: impl FnOnce(&mut AppConfigurationInner) -> TResult,
    ) -> TResult {
        let mut inner = self.inner.write().await;
        func(&mut inner)
    }


    pub async fn get_ssl_certified_key(
        &self,
        listen_port: u16,
        server_name: &str,
    ) -> Result<Arc<CertifiedKey>, String> {
        if let Some(port_configuration) = self.http_endpoints.get(&listen_port) {
            let ssl_certificate_id = port_configuration.get_ssl_certificate(server_name);

            if ssl_certificate_id.is_none() {
                return Err(format!(
                    "No matching configuration for server_name {} on port {}.",
                    server_name, listen_port
                ));
            }

            let ssl_certificate_id = ssl_certificate_id.unwrap();

            if ssl_certificate_id.as_str() == SELF_SIGNED_CERT_NAME {
                return Ok(Arc::new(crate::self_signed_cert::generate(
                    server_name.to_string(),
                )?));
            }

            if let Some(key) = self
                .ssl_certificates_cache
                .get_certified_key(&ssl_certificate_id)
                .await
            {
                return Ok(key);
            } else {
                return Err(format!(
                    "Can not find ssl_certified_key for port: {}",
                    listen_port
                ));
            }
        } else {
            Err(format!(
                "Can not find ssl_certified_key for port: {}",
                listen_port
            ))
        }
    }

    pub fn get_http_endpoint_info(
        &self,
        listen_port: u16,
        server_name: &str,
    ) -> Result<Arc<HttpEndpointInfo>, String> {
        if let Some(listen_port_config) = self.http_endpoints.get(&listen_port) {
            for endpoint_info in &listen_port_config.endpoint_info {
                if endpoint_info.is_my_endpoint(server_name) {
                    return Ok(endpoint_info.clone());
                }
            }
        }

        Err(format!("Not port is listening at port: {}", listen_port))
    }
     */
}
