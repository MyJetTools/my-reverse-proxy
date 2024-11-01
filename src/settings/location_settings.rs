use std::{collections::HashMap, str::FromStr};

use serde::*;

use crate::{configurations::*, variables_reader::VariablesReader};

use super::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocationSettings {
    pub path: Option<String>,
    pub proxy_pass_to: String,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    pub domain_name: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
    pub default_file: Option<String>,
    pub status_code: Option<u16>,
    pub content_type: Option<String>,
    pub body: Option<String>,
    pub whitelisted_ip: Option<String>,
    pub compress: Option<bool>,
}

impl LocationSettings {
    fn get_status_code(&self, endpoint_str: &str) -> Result<u16, String> {
        match self.status_code {
            Some(status_code) => Ok(status_code),
            None => Err(format!(
                "status_code is required for static content for endpoint {}",
                endpoint_str
            )),
        }
    }

    pub fn get_proxy_pass(
        &self,
        endpoint_str: &str,
        variables: VariablesReader,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
    ) -> Result<ProxyPassTo, String> {
        let proxy_pass_to =
            crate::populate_variable::populate_variable(self.proxy_pass_to.trim(), variables);

        if proxy_pass_to.as_str().trim() == "static" {
            return Ok(ProxyPassTo::Static(StaticContentModel {
                status_code: self.get_status_code(endpoint_str)?,
                content_type: self.content_type.clone(),
                body: if let Some(body) = self.body.as_ref() {
                    body.as_bytes().to_vec()
                } else {
                    vec![]
                },
            }));
        }

        if proxy_pass_to.as_str().starts_with(SSH_PREFIX) {
            return Ok(ProxyPassTo::Ssh(SshProxyPassModel {
                ssh_config: SshConfiguration::parse(
                    proxy_pass_to.as_str(),
                    &ssh_configs,
                    variables,
                )?,
                http2: self.get_type().is_protocol_http2(),
                default_file: self.default_file.clone(),
            }));
        }

        if proxy_pass_to.as_str().starts_with("http") {
            if self.get_type().is_protocol_http2() {
                return Ok(ProxyPassTo::Http2(RemoteHost::new(
                    proxy_pass_to.to_string(),
                )));
            } else {
                return Ok(ProxyPassTo::Http1(RemoteHost::new(
                    proxy_pass_to.to_string(),
                )));
            }
        }

        if proxy_pass_to.as_str().starts_with("~")
            || proxy_pass_to.as_str().starts_with("/")
            || proxy_pass_to.as_str().starts_with(".")
        {
            return Ok(ProxyPassTo::LocalPath(LocalPathModel {
                local_path: LocalFilePath::new(proxy_pass_to.to_string()),
                default_file: self.default_file.clone(),
            }));
        }

        Ok(ProxyPassTo::Tcp(
            std::net::SocketAddr::from_str(proxy_pass_to.as_str()).unwrap(),
        ))
    }

    pub fn get_type(&self) -> HttpType {
        match self.location_type.as_ref() {
            Some(location_type) => match location_type.as_str() {
                "http" => HttpType::Http1,
                "http2" => HttpType::Http2,
                "https1" => HttpType::Https1,
                "https2" => HttpType::Https2,
                _ => HttpType::Http1,
            },
            None => HttpType::Http1,
        }
    }

    pub fn get_compress(&self) -> bool {
        match self.compress {
            Some(compress) => compress,
            None => false,
        }
    }

    /*
    pub fn is_http2(&self) -> Result<bool, String> {
        if let Some(location_type) = self.location_type.as_ref() {
            match location_type.as_str() {
                "http" => return Ok(false),
                "http2" => return Ok(true),
                _ => return Err(format!("Unknown remote location type {}", location_type)),
            }
        }

        Ok(false)
    }
     */
}
