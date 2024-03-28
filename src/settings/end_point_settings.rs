use std::collections::HashMap;

use serde::*;

use crate::{
    http_proxy_pass::{HttpType, ProxyPassEndpointInfo},
    types::WhiteListedIpList,
};

use super::{
    EndpointTemplateSettings, EndpointType, GoogleAuthSettings, LocationSettings,
    ModifyHttpHeadersSettings, SshConfigSettings, SslCertificateId,
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
    pub template_id: Option<String>,
    pub allowed_users: Option<String>,
}

impl EndpointSettings {
    fn get_debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }

    pub fn get_modify_http_headers(
        &self,
        endpoint_templates: &Option<HashMap<String, EndpointTemplateSettings>>,
    ) -> Option<ModifyHttpHeadersSettings> {
        if self.modify_http_headers.is_none() {
            return self.modify_http_headers.clone();
        }

        if let Some(template_id) = self.template_id.as_ref() {
            if let Some(endpoint_templates) = endpoint_templates {
                if let Some(template) = endpoint_templates.get(template_id) {
                    return template.modify_http_headers.clone();
                }
            }
        }

        None
    }

    pub fn get_white_listed_ip(
        &self,
        endpoint_templates: &Option<HashMap<String, EndpointTemplateSettings>>,
    ) -> Option<String> {
        if let Some(whitelisted_ip) = self.whitelisted_ip.as_ref() {
            return Some(whitelisted_ip.to_string());
        }

        let template_id = self.template_id.as_deref()?;
        let endpoint_templates = endpoint_templates.as_ref()?;

        let template = endpoint_templates.get(template_id)?;

        template.whitelisted_ip.clone()
    }

    fn get_google_auth_settings(
        &self,
        endpoint_template: Option<&EndpointTemplateSettings>,
        g_auth_settings: &Option<HashMap<String, GoogleAuthSettings>>,
    ) -> Result<Option<GoogleAuthSettings>, String> {
        let mut g_auth_id = self.google_auth.as_ref();

        if g_auth_id.is_none() {
            if let Some(endpoint_template) = endpoint_template {
                g_auth_id = endpoint_template.google_auth.as_ref();
            }
        }

        if g_auth_id.is_none() {
            return Ok(None);
        }

        let g_auth_id = g_auth_id.unwrap();

        if let Some(g_auth_settings) = g_auth_settings {
            if let Some(result) = g_auth_settings.get(g_auth_id) {
                return Ok(Some(result.clone()));
            }
        }

        Err(format!(
            "Can not find google_auth with id '{}' for endpoint",
            g_auth_id
        ))
    }

    fn get_ssl_id(
        &self,
        endpoint_template: Option<&EndpointTemplateSettings>,
    ) -> Option<SslCertificateId> {
        if let Some(ssl_certificate) = &self.ssl_certificate {
            return Some(SslCertificateId::new(ssl_certificate.to_string()));
        }

        let endpoint_template = endpoint_template?;

        if let Some(ssl_certificate) = endpoint_template.ssl_certificate.as_ref() {
            return Some(SslCertificateId::new(ssl_certificate.clone()));
        }

        None
    }

    fn get_client_certificate_id(
        &self,
        endpoint_template: Option<&EndpointTemplateSettings>,
    ) -> Option<SslCertificateId> {
        if let Some(client_certificate_ca) = &self.client_certificate_ca {
            return Some(SslCertificateId::new(client_certificate_ca.to_string()));
        }

        let endpoint_template = endpoint_template?;

        if let Some(client_certificate_ca) = endpoint_template.client_certificate_ca.as_ref() {
            return Some(SslCertificateId::new(client_certificate_ca.clone()));
        }

        None
    }

    pub fn get_type(
        &self,
        host: &str,
        locations: &[LocationSettings],
        endpoint_templates: &Option<HashMap<String, EndpointTemplateSettings>>,
        variables: &Option<HashMap<String, String>>,
        ssh_config: &Option<HashMap<String, SshConfigSettings>>,
        g_auth_settings: &Option<HashMap<String, GoogleAuthSettings>>,
    ) -> Result<EndpointType, String> {
        let endpoint_template = if let Some(template_id) = self.template_id.as_ref() {
            match endpoint_templates {
                Some(endpoint_templates) => match endpoint_templates.get(template_id) {
                    Some(template) => Some(template),
                    None => {
                        return Err(format!(
                            "Can not find template with id '{}' for endpoint",
                            template_id
                        ));
                    }
                },
                None => {
                    return Err(format!(
                        "Can not find template with id '{}' for google_auth",
                        template_id
                    ));
                }
            }
        } else {
            None
        };

        let g_auth = self.get_google_auth_settings(endpoint_template, g_auth_settings)?;

        match self.endpoint_type.as_str() {
            HTTP1_ENDPOINT_TYPE => Ok(EndpointType::Http1(ProxyPassEndpointInfo::new(
                host.to_string(),
                HttpType::Http1,
                self.get_debug(),
                g_auth,
            ))),
            "https" => {
                if let Some(ssl_id) = self.get_ssl_id(endpoint_template) {
                    return Ok(EndpointType::Https {
                        endpoint_info: ProxyPassEndpointInfo::new(
                            host.to_string(),
                            HttpType::Https1,
                            self.get_debug(),
                            g_auth,
                        ),
                        ssl_id,
                        client_ca_id: self.get_client_certificate_id(endpoint_template),
                    });
                } else {
                    panic!("Host '{}' has https location without ssl certificate", host);
                }
            }
            "https2" => {
                if let Some(ssl_id) = self.get_ssl_id(endpoint_template) {
                    return Ok(EndpointType::Https2 {
                        endpoint_info: ProxyPassEndpointInfo::new(
                            host.to_string(),
                            HttpType::Https2,
                            self.get_debug(),
                            g_auth,
                        ),
                        ssl_id,
                        client_ca_id: self.get_client_certificate_id(endpoint_template),
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
                        let mut whitelisted_ip = WhiteListedIpList::new();

                        whitelisted_ip.apply(self.whitelisted_ip.as_deref());

                        return Ok(EndpointType::Tcp {
                            remote_addr,
                            debug: self.get_debug(),
                            whitelisted_ip,
                        });
                    }
                }
            }
            _ => panic!("Unknown location type: '{}'", self.endpoint_type),
        }
    }
}
