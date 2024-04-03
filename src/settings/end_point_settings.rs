use std::{collections::HashMap, sync::Arc};

use serde::*;

use crate::{
    app::AppContext,
    app_configuration::{
        EndpointType, HttpEndpointInfo, HttpType, ProxyPassLocationConfig, TcpEndpointHostConfig,
        TcpOverSshEndpointHostConfig,
    },
    http_proxy_pass::AllowedUserList,
    types::WhiteListedIpList,
};

use super::{
    EndpointHttpHostString, EndpointTemplateSettings, GlobalSettings, GoogleAuthSettings,
    HttpEndpointModifyHeadersSettings, LocationSettings, ModifyHttpHeadersSettings,
    SshConfigSettings, SslCertificateId,
};

const HTTP1_ENDPOINT_TYPE: &str = "http";
const HTTP2_ENDPOINT_TYPE: &str = "http2";

const HTTPS1_ENDPOINT_TYPE: &str = "https";
const HTTPS2_ENDPOINT_TYPE: &str = "https2";

const TCP_ENDPOINT_TYPE: &str = "tcp";

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
    pub fn get_debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }

    pub fn get_http_endpoint_modify_headers_settings(
        &self,
        global_settings: &Option<GlobalSettings>,
        endpoint_template_settings: Option<&EndpointTemplateSettings>,
    ) -> HttpEndpointModifyHeadersSettings {
        let mut result = HttpEndpointModifyHeadersSettings::default();

        if let Some(global_settings) = global_settings {
            if let Some(all_http_endpoints) = global_settings.all_http_endpoints.as_ref() {
                if let Some(modify_headers) = all_http_endpoints.modify_http_headers.as_ref() {
                    result.global_modify_headers_settings = Some(modify_headers.clone());
                }
            }
        }

        if let Some(modify_headers) = self.get_modify_http_headers(endpoint_template_settings) {
            result.endpoint_modify_headers_settings = Some(modify_headers.clone());
        }

        result
    }

    pub fn get_modify_http_headers(
        &self,
        endpoint_templates: Option<&EndpointTemplateSettings>,
    ) -> Option<ModifyHttpHeadersSettings> {
        if self.modify_http_headers.is_none() {
            return self.modify_http_headers.clone();
        }

        if let Some(endpoint_template_settings) = endpoint_templates {
            return endpoint_template_settings.modify_http_headers.clone();
        }

        None
    }

    pub fn get_white_listed_ip(
        &self,
        endpoint_template_settings: Option<&EndpointTemplateSettings>,
    ) -> Option<String> {
        if let Some(whitelisted_ip) = self.whitelisted_ip.as_ref() {
            return Some(whitelisted_ip.to_string());
        }

        let endpoint_template_settings = endpoint_template_settings?;

        endpoint_template_settings.whitelisted_ip.clone()
    }

    pub fn get_google_auth_settings(
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

    pub fn get_ssl_id(
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

    pub fn get_client_certificate_id(
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

    pub fn get_endpoint_template<'s>(
        &'s self,
        endpoint_templates: &'s Option<HashMap<String, EndpointTemplateSettings>>,
    ) -> Result<Option<&'s EndpointTemplateSettings>, String> {
        if let Some(template_id) = self.template_id.as_ref() {
            match endpoint_templates {
                Some(endpoint_templates) => match endpoint_templates.get(template_id) {
                    Some(template) => return Ok(Some(template)),
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
            return Ok(None);
        }
    }

    pub fn get_type(
        &self,
        host: EndpointHttpHostString,
        endpoint_settings: &EndpointSettings,
        locations: &[LocationSettings],
        endpoint_template_settings: Option<&EndpointTemplateSettings>,
        variables: &Option<HashMap<String, String>>,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
        g_auth_settings: &Option<HashMap<String, GoogleAuthSettings>>,
        allowed_user_list: Option<Arc<AllowedUserList>>,
        global_settings: &Option<GlobalSettings>,
        app: &AppContext,
    ) -> Result<EndpointType, String> {
        let g_auth = self.get_google_auth_settings(endpoint_template_settings, g_auth_settings)?;

        match self.endpoint_type.as_str() {
            HTTP1_ENDPOINT_TYPE => {
                let locations = convert_to_http_locations(
                    &host,
                    locations,
                    endpoint_settings,
                    endpoint_template_settings,
                    variables,
                    ssh_configs,
                    app,
                )?;

                return Ok(EndpointType::Http(HttpEndpointInfo::new(
                    host,
                    HttpType::Http1,
                    self.get_debug(),
                    g_auth,
                    self.get_ssl_id(endpoint_template_settings),
                    None,
                    locations,
                    allowed_user_list,
                    self.get_http_endpoint_modify_headers_settings(
                        global_settings,
                        endpoint_template_settings,
                    ),
                )));
            }
            HTTP2_ENDPOINT_TYPE => {
                let locations = convert_to_http_locations(
                    &host,
                    locations,
                    endpoint_settings,
                    endpoint_template_settings,
                    variables,
                    ssh_configs,
                    app,
                )?;

                return Ok(EndpointType::Http(HttpEndpointInfo::new(
                    host,
                    HttpType::Http2,
                    self.get_debug(),
                    g_auth,
                    self.get_ssl_id(endpoint_template_settings),
                    None,
                    locations,
                    allowed_user_list,
                    self.get_http_endpoint_modify_headers_settings(
                        global_settings,
                        endpoint_template_settings,
                    ),
                )));
            }
            HTTPS1_ENDPOINT_TYPE => {
                let locations = convert_to_http_locations(
                    &host,
                    locations,
                    endpoint_settings,
                    endpoint_template_settings,
                    variables,
                    ssh_configs,
                    app,
                )?;

                return Ok(EndpointType::Http(HttpEndpointInfo::new(
                    host,
                    HttpType::Https1,
                    self.get_debug(),
                    g_auth,
                    self.get_ssl_id(endpoint_template_settings),
                    self.get_client_certificate_id(endpoint_template_settings),
                    locations,
                    allowed_user_list,
                    self.get_http_endpoint_modify_headers_settings(
                        global_settings,
                        endpoint_template_settings,
                    ),
                )));
            }

            HTTPS2_ENDPOINT_TYPE => {
                let locations = convert_to_http_locations(
                    &host,
                    locations,
                    endpoint_settings,
                    endpoint_template_settings,
                    variables,
                    ssh_configs,
                    app,
                )?;

                return Ok(EndpointType::Http(HttpEndpointInfo::new(
                    host,
                    HttpType::Https2,
                    self.get_debug(),
                    g_auth,
                    self.get_ssl_id(endpoint_template_settings),
                    self.get_client_certificate_id(endpoint_template_settings),
                    locations,
                    allowed_user_list,
                    self.get_http_endpoint_modify_headers_settings(
                        global_settings,
                        endpoint_template_settings,
                    ),
                )));
            }

            TCP_ENDPOINT_TYPE => {
                if locations.len() != 1 {
                    panic!(
                        "Tcp Host '{}' has {} locations to proxy_pass. Tcp Host must have 1 location",
                        host.as_str(),
                        locations.len()
                    );
                }

                let location_settings = locations.get(0).unwrap();

                match location_settings.get_proxy_pass(host.as_str(), variables, ssh_configs)? {
                    super::ProxyPassTo::Http(_) => {
                        return Err(
                            "It is not possible to serve remote http content over tcp endpoint"
                                .to_string(),
                        );
                    }

                    super::ProxyPassTo::Http2(_) => {
                        return Err(
                            "It is not possible to serve remote http2 content over tcp endpoint"
                                .to_string(),
                        );
                    }
                    super::ProxyPassTo::Static(_) => {
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
                    super::ProxyPassTo::Ssh(model) => match model.ssh_config.remote_content {
                        super::SshContent::RemoteHost(remote_host) => {
                            return Ok(EndpointType::TcpOverSsh(
                                TcpOverSshEndpointHostConfig {
                                    ssh_credentials: model.ssh_config.credentials.clone(),
                                    remote_host: Arc::new(remote_host),
                                    debug: self.get_debug(),
                                    host,
                                }
                                .into(),
                            ));
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

                        return Ok(EndpointType::Tcp(
                            TcpEndpointHostConfig {
                                remote_addr,
                                debug: self.get_debug(),
                                whitelisted_ip,
                                host,
                            }
                            .into(),
                        ));
                    }
                }
            }
            _ => panic!("Unknown location type: '{}'", self.endpoint_type),
        }
    }
}

fn convert_to_http_locations(
    host: &EndpointHttpHostString,
    src: &[LocationSettings],
    endpoint_settings: &EndpointSettings,
    endpoint_template_settings: Option<&EndpointTemplateSettings>,
    variables: &Option<HashMap<String, String>>,
    ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
    app: &AppContext,
) -> Result<Vec<Arc<ProxyPassLocationConfig>>, String> {
    let mut result = Vec::with_capacity(src.len());

    for location_settings in src {
        let location_path = if let Some(location) = &location_settings.path {
            location.to_string()
        } else {
            "/".to_string()
        };

        let mut whitelisted_ip = WhiteListedIpList::new();
        whitelisted_ip.apply(
            endpoint_settings
                .get_white_listed_ip(endpoint_template_settings)
                .as_deref(),
        );
        whitelisted_ip.apply(location_settings.whitelisted_ip.as_deref());

        result.push(
            ProxyPassLocationConfig::new(
                app.get_id(),
                location_path,
                location_settings.modify_http_headers.clone(),
                whitelisted_ip,
                location_settings.get_proxy_pass(host.as_str(), variables, ssh_configs)?,
            )
            .into(),
        );
    }

    Ok(result)
}
