use serde::*;

use super::*;

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

    /*
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
    */
    pub fn get_endpoint_type(&self) -> Result<EndpointTypeSettings, String> {
        let result = match self.endpoint_type.as_str() {
            HTTP1_ENDPOINT_TYPE => EndpointTypeSettings::Http1,
            HTTP2_ENDPOINT_TYPE => EndpointTypeSettings::Http2,
            HTTPS1_ENDPOINT_TYPE => EndpointTypeSettings::Https1,
            "http1" => EndpointTypeSettings::Https1,
            HTTPS2_ENDPOINT_TYPE => EndpointTypeSettings::Https2,
            TCP_ENDPOINT_TYPE => EndpointTypeSettings::Tcp,
            _ => return Err(format!("Unknown endpoint type: '{}'", self.endpoint_type)),
        };

        Ok(result)
    }

    /*
    pub fn get_type(
        &self,
        host: EndpointHttpHostString,
        endpoint_settings: &EndpointSettings,
        locations: &[LocationSettings],
        endpoint_template_settings: Option<&EndpointTemplateSettings>,
        variables: VariablesReader,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
        g_auth_settings: &Option<HashMap<String, GoogleAuthSettings>>,
        allowed_user_list: Option<Arc<AllowedUserList>>,
        global_settings: &Option<GlobalSettings>,
        app: &AppContext,
    ) -> Result<EndpointType, String> {
        let g_auth =
            self.get_google_auth_settings(endpoint_template_settings, g_auth_settings, variables)?;

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
                    super::ProxyPassTo::Http1(_) => {
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
                        SshContent::RemoteHost(remote_host) => {
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
                        SshContent::FilePath(_) => {
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
     */
}

/*
fn convert_to_http_locations(
    host: &EndpointHttpHostString,
    src: &[LocationSettings],
    endpoint_settings: &EndpointSettings,
    endpoint_template_settings: Option<&EndpointTemplateSettings>,
    variables: VariablesReader,
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
                location_settings.domain_name.clone(),
                location_settings.get_type(),
                location_settings.get_compress(),
            )
            .into(),
        );
    }

    Ok(result)
}
 */
