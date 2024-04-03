use std::{collections::BTreeMap, sync::Arc};

use tokio_rustls::rustls::sign::CertifiedKey;

use crate::ssl::*;

use super::{HttpEndpointInfo, ListenPortConfiguration};

pub struct AppConfiguration {
    pub client_certificates_cache: ClientCertificatesCache,
    pub ssl_certificates_cache: SslCertificatesCache,
    pub listen_ports: BTreeMap<u16, ListenPortConfiguration>,
}

impl AppConfiguration {
    /*
    pub fn get_http_locations(
        &self,
        endpoint_info: &HttpEndpointInfo,
        req: &HttpRequestBuilder,
        is_https: bool,
    ) -> Result<(Vec<ProxyPassLocation>, Option<Arc<AllowedUserList>>), String> {
        if let Some(endpoint_type) = self
            .listen_ports
            .get(&endpoint_info.host_endpoint.get_port())
        {
            if let Some((locations, allowed_user_list)) =
                endpoint_type.get_my_http_locations(req, is_https)
            {
                let mut result = Vec::with_capacity(locations.len());

                for location_config in locations {
                    result.push(ProxyPassLocation::new(location_config.clone()));
                }

                return Ok((result, allowed_user_list));
            }
        }

        let http_type = if is_https { "https" } else { "http" };

        Err(format!(
            "Can not get http locations for {} endpoint: {}",
            http_type,
            endpoint_info.as_str()
        ))
    } */

    pub async fn get_ssl_certified_key(
        &self,
        listen_port: u16,
    ) -> Result<Arc<CertifiedKey>, String> {
        if let Some(port_configuration) = self.listen_ports.get(&listen_port) {
            let ssl_certificate_id = port_configuration.get_ssl_certificate();

            if ssl_certificate_id.is_none() {
                return Err(format!(
                    "Can not find ssl_certified_key for port: {}",
                    listen_port
                ));
            }

            let ssl_certificate_id = ssl_certificate_id.unwrap();

            if let Some(key) = self
                .ssl_certificates_cache
                .get_certified_key(&ssl_certificate_id)
            {
                return Ok(key);
            } else {
                return Err(format!(
                    "Can not find ssl_certified_key for port: {}",
                    listen_port
                ));
            }
        } else {
            panic!("Can not find ssl_certified_key for port: {}", listen_port);
        }
    }

    pub async fn get_ssl_key(&self, listen_port: u16) -> Result<Arc<SslCertificate>, String> {
        if let Some(port_configuration) = self.listen_ports.get(&listen_port) {
            let ssl_certificate_id = port_configuration.get_ssl_certificate();

            if ssl_certificate_id.is_none() {
                return Err(format!(
                    "Can not find ssl_certified_key for port: {}",
                    listen_port
                ));
            }

            let ssl_certificate_id = ssl_certificate_id.unwrap();

            if let Some(result) = self.ssl_certificates_cache.get_ssl_key(&ssl_certificate_id) {
                return Ok(result);
            } else {
                return Err(format!(
                    "Can not find ssl_certified_key for port: {}",
                    listen_port
                ));
            }
        } else {
            panic!("Can not find ssl_certified_key for port: {}", listen_port);
        }
    }

    pub async fn get_http_endpoint_info(
        &self,
        listen_port: u16,
        server_name: &str,
    ) -> Result<Arc<HttpEndpointInfo>, String> {
        if let Some(listen_port_config) = self.listen_ports.get(&listen_port) {
            return match listen_port_config {
                ListenPortConfiguration::Http(http_listen_port_configuration) => {
                    for endpoint_info in &http_listen_port_configuration.endpoint_info {
                        if endpoint_info.is_my_endpoint(server_name) {
                            return Ok(endpoint_info.clone());
                        }
                    }

                    return Err(format!(
                        "Can not find http endpoint info for port: {} and server name: {}",
                        listen_port, server_name
                    ));
                }
                ListenPortConfiguration::Tcp(_) => Err(format!(
                    "Can not get Http endpoint configuration from tcp endpoint port {}",
                    listen_port
                )),

                ListenPortConfiguration::TcpOverSsh(_) => Err(format!(
                    "Can not get Http endpoint configuration from tcp over ssh endpoint port {}",
                    listen_port
                )),
            };
        }

        Err(format!("Not port is listening at port: {}", listen_port))
    }
}
