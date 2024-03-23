use std::collections::HashMap;

use serde::*;

use crate::http_proxy_pass::ProxyPassError;

use super::{EndpointType, LocationSettings, ModifyHttpHeadersSettings, SslCertificateId};

const HTTP1_ENDPOINT_TYPE: &str = "http";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EndpointSettings {
    #[serde(rename = "type")]
    pub endpoint_type: String,
    pub ssl_certificate: Option<String>,
    pub client_certificate_ca: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
}

impl EndpointSettings {
    pub fn get_type(
        &self,
        host: &str,
        locations: &[LocationSettings],
        variables: &Option<HashMap<String, String>>,
    ) -> Result<EndpointType, ProxyPassError> {
        match self.endpoint_type.as_str() {
            HTTP1_ENDPOINT_TYPE => Ok(EndpointType::Http1(host.to_string())),
            "https" => {
                if let Some(ssl_certificate) = &self.ssl_certificate {
                    return Ok(EndpointType::Https {
                        host_str: host.to_string(),
                        ssl_id: SslCertificateId::new(ssl_certificate.to_string()),
                        client_ca_id: self
                            .client_certificate_ca
                            .as_ref()
                            .map(|x| SslCertificateId::new(x.to_string())),
                    });
                } else {
                    panic!("Host '{}' has https location without ssl certificate", host);
                }
            }
            "https2" => {
                if let Some(ssl_certificate) = &self.ssl_certificate {
                    return Ok(EndpointType::Https2 {
                        host_str: host.to_string(),
                        ssl_id: SslCertificateId::new(ssl_certificate.to_string()),
                        client_ca_id: self
                            .client_certificate_ca
                            .as_ref()
                            .map(|x| SslCertificateId::new(x.to_string())),
                    });
                } else {
                    panic!("Host '{}' has https location without ssl certificate", host);
                }
            }
            "http2" => return Ok(EndpointType::Http2(host.to_string())),
            "tcp" => {
                if locations.len() != 1 {
                    panic!(
                        "Tcp Host '{}' has {} locations to proxy_pass. Tcp Host must have 1 location",
                        host,
                        locations.len()
                    );
                }

                let location_settings = locations.get(0).unwrap();

                match location_settings.get_proxy_pass(variables) {
                    super::ProxyPassTo::Http(_) => {
                        return Err(ProxyPassError::CanNotReadSettingsConfiguration(
                            "It is not possible to serve remote http content over tcp endpoint"
                                .to_string(),
                        ));
                    }
                    super::ProxyPassTo::LocalPath(_) => {
                        return Err(ProxyPassError::CanNotReadSettingsConfiguration(
                            "It is not possible to serve local path content over tcp endpoint"
                                .to_string(),
                        ));
                    }
                    super::ProxyPassTo::Ssh(ssh_config) => match ssh_config.remote_content {
                        super::SshContent::RemoteHost(remote_host) => {
                            return Ok(EndpointType::TcpOverSsh {
                                ssh_credentials: ssh_config.credentials,
                                remote_host,
                            });
                        }
                        super::SshContent::FilePath(_) => {
                            return Err(ProxyPassError::CanNotReadSettingsConfiguration(
                                "It is not possible to serve remote ssh path content over tcp endpoint"
                                    .to_string(),
                            ));
                        }
                    },
                    super::ProxyPassTo::Tcp(socket_addr) => {
                        return Ok(EndpointType::Tcp(socket_addr));
                    }
                }

                /*
                 Ok(result) => return Ok(result),
                   Err(err) => {
                       return Err(ProxyPassError::CanNotReadSettingsConfiguration(format!(
                           "Invalid proxy_pass_to {} for tcp endpoint {}. {}",
                           location_settings.proxy_pass_to, host, err
                       )));
                   }
                */
            }
            _ => panic!("Unknown location type: '{}'", self.endpoint_type),
        }
    }
}
