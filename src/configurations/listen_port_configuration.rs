use std::sync::Arc;

use super::*;

pub enum ListenPortConfiguration {
    Http(HttpListenPortConfiguration),
    Tcp(Arc<TcpEndpointHostConfig>),
    TcpOverSsh(Arc<TcpOverSshEndpointHostConfig>),
}

impl ListenPortConfiguration {
    pub fn get_endpoint_host_as_str(&self) -> &str {
        match self {
            ListenPortConfiguration::Http(http_listen_port_configuration) => {
                for endpoint_info in http_listen_port_configuration.endpoint_info.iter() {
                    return endpoint_info.host_endpoint.as_str();
                }

                "none"
            }
            ListenPortConfiguration::Tcp(tcp_endpoint_host_config) => {
                tcp_endpoint_host_config.host.as_str()
            }
            ListenPortConfiguration::TcpOverSsh(tcp_over_ssh_endpoint_host_config) => {
                tcp_over_ssh_endpoint_host_config.host.as_str()
            }
        }
    }

    pub fn add_http_endpoint_info(
        &mut self,
        host_str: &str,
        http_endpoint_info: HttpEndpointInfo,
    ) -> Result<(), String> {
        match self {
            ListenPortConfiguration::Http(http) => {
                http.endpoint_info.push(Arc::new(http_endpoint_info));
            }
            ListenPortConfiguration::Tcp(_) => {
                return Err(format!(
                    "Cannot add http endpoint {} info to a non-http endpoint {}",
                    host_str,
                    http_endpoint_info.host_endpoint.as_str()
                ));
            }
            ListenPortConfiguration::TcpOverSsh(_) => {
                return Err(format!(
                    "Cannot add http endpoint {} info to tcp over ssh a non-http endpoint {}",
                    host_str,
                    http_endpoint_info.host_endpoint.as_str()
                ));
            }
        }

        Ok(())
    }

    /*
    pub fn get_client_certificates(&self) -> Vec<&SslCertificateId> {
        let mut result = vec![];
        match self {
            ListenPortConfiguration::Http(port_configuration) => {
                for endpoint_info in &port_configuration.endpoint_info {
                    if let Some(ssl_id) = endpoint_info.client_certificate_id.as_ref() {
                        result.push(ssl_id);
                    }
                }

                result
            }
            ListenPortConfiguration::Tcp(_) => result,
            ListenPortConfiguration::TcpOverSsh(_) => result,
        }
    }
     */
}
