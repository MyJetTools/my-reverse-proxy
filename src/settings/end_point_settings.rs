use std::collections::HashMap;

use serde::*;

use crate::http_proxy_pass::{HttpType, ProxyPassEndpointInfo};

use super::{
    EndpointType, GoogleAuthSettings, LocationSettings, ModifyHttpHeadersSettings,
    SshConfigSettings, SslCertificateId,
};

const HTTP1_ENDPOINT_TYPE: &str = "http";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EndpointSettings {
    #[serde(rename = "type")]
    pub endpoint_type: String,
    pub ssl_certificate: Option<String>,
    pub client_certificate_ca: Option<String>,
    pub google_auth: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
    pub debug: Option<bool>,
    pub whitelisted_ip: Option<String>,
}

impl EndpointSettings {
    fn get_debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }
    pub fn get_type(
        &self,
        host: &str,
        locations: &[LocationSettings],
        variables: &Option<HashMap<String, String>>,
        ssh_config: &Option<HashMap<String, SshConfigSettings>>,
        g_auth_settings: &Option<HashMap<String, GoogleAuthSettings>>,
    ) -> Result<EndpointType, String> {
        let g_auth = if let Some(g_auth_id) = self.google_auth.as_ref() {
            if let Some(g_auth_settings) = g_auth_settings {
                g_auth_settings.get(g_auth_id.as_str()).cloned()
            } else {
                None
            }
        } else {
            None
        };

        match self.endpoint_type.as_str() {
            HTTP1_ENDPOINT_TYPE => Ok(EndpointType::Http1(ProxyPassEndpointInfo::new(
                host.to_string(),
                HttpType::Http1,
                self.get_debug(),
                g_auth,
            ))),
            "https" => {
                if let Some(ssl_certificate) = &self.ssl_certificate {
                    return Ok(EndpointType::Https {
                        endpoint_info: ProxyPassEndpointInfo::new(
                            host.to_string(),
                            HttpType::Https1,
                            self.get_debug(),
                            g_auth,
                        ),
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
                        endpoint_info: ProxyPassEndpointInfo::new(
                            host.to_string(),
                            HttpType::Https2,
                            self.get_debug(),
                            g_auth,
                        ),
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
            "http2" => {
                return Ok(EndpointType::Http2(ProxyPassEndpointInfo::new(
                    host.to_string(),
                    HttpType::Http2,
                    self.get_debug(),
                    g_auth,
                )))
            }
            "tcp" => {
                if locations.len() != 1 {
                    panic!(
                        "Tcp Host '{}' has {} locations to proxy_pass. Tcp Host must have 1 location",
                        host,
                        locations.len()
                    );
                }

                let location_settings = locations.get(0).unwrap();

                match location_settings.get_proxy_pass(variables, ssh_config)? {
                    super::ProxyPassTo::Http(_) => {
                        return Err(
                            "It is not possible to serve remote http content over tcp endpoint"
                                .to_string(),
                        );
                    }
                    super::ProxyPassTo::Static => {
                        return Err(
                            "It is not possible to serve static content over tcp endpoint"
                                .to_string(),
                        );
                    }
                    super::ProxyPassTo::LocalPath(_) => {
                        return Err(
                            "It is not possible to serve local path content over tcp endpoint"
                                .to_string(),
                        );
                    }
                    super::ProxyPassTo::Ssh(ssh_config) => match ssh_config.remote_content {
                        super::SshContent::RemoteHost(remote_host) => {
                            return Ok(EndpointType::TcpOverSsh {
                                debug: self.get_debug(),
                                ssh_credentials: ssh_config.credentials,
                                remote_host,
                            });
                        }
                        super::SshContent::FilePath(_) => {
                            return Err(
                                "It is not possible to serve remote ssh path content over tcp endpoint"
                                    .to_string(),
                            );
                        }
                    },
                    super::ProxyPassTo::Tcp(remote_addr) => {
                        return Ok(EndpointType::Tcp {
                            remote_addr,
                            debug: self.get_debug(),
                        });
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
