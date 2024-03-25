use std::collections::HashMap;

use serde::*;

use crate::{
    app::AppContext,
    http_content_source::{
        LocalPathContentSrc, PathOverSshContentSource, RemoteHttpContentSource, StaticContentSrc,
    },
    http_proxy_pass::HttpProxyPassContentSource,
};

use super::{
    HttpProxyPassRemoteEndpoint, ModifyHttpHeadersSettings, ProxyPassTo, RemoteHost,
    SshConfigSettings,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocationSettings {
    pub path: Option<String>,
    pub proxy_pass_to: String,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
    pub default_file: Option<String>,
    pub status_code: Option<u16>,
    pub content_type: Option<String>,
    pub body: Option<String>,
}

impl LocationSettings {
    pub fn get_proxy_pass(
        &self,
        variables: &Option<HashMap<String, String>>,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
    ) -> Result<ProxyPassTo, String> {
        let proxy_pass_to =
            crate::populate_variable::populate_variable(self.proxy_pass_to.trim(), variables);
        ProxyPassTo::from_str(proxy_pass_to, ssh_configs)
    }

    pub fn get_http_content_source<'s>(
        &'s self,
        app: &AppContext,
        host: &str,
        location_id: i64,
        variables: &Option<HashMap<String, String>>,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
    ) -> Result<Option<HttpProxyPassContentSource>, String> {
        let proxy_pass_to = self.get_proxy_pass(variables, ssh_configs)?;

        match proxy_pass_to {
            ProxyPassTo::Static => {
                return Ok(HttpProxyPassContentSource::Static(StaticContentSrc::new(
                    match self.status_code {
                        Some(status_code) => status_code,
                        None => panic!(
                            "status_code is required for static content in host {}",
                            host
                        ),
                    },
                    self.content_type.clone(),
                    if let Some(body) = self.body.as_ref() {
                        body.as_bytes().to_vec()
                    } else {
                        vec![]
                    },
                ))
                .into())
            }
            ProxyPassTo::Http(remote_host) => {
                if self.is_http2()? {
                    return Ok(
                        HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                            location_id,
                            HttpProxyPassRemoteEndpoint::Http2(RemoteHost::new(
                                remote_host.to_string(),
                            )),
                        ))
                        .into(),
                    );
                } else {
                    return Ok(
                        HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                            location_id,
                            HttpProxyPassRemoteEndpoint::Http(RemoteHost::new(
                                remote_host.to_string(),
                            )),
                        ))
                        .into(),
                    );
                }
            }
            ProxyPassTo::LocalPath(file_path) => {
                return Ok(
                    HttpProxyPassContentSource::LocalPath(LocalPathContentSrc::new(
                        file_path,
                        self.default_file.clone(),
                    ))
                    .into(),
                );
            }
            ProxyPassTo::Ssh(ssh_configuration) => match ssh_configuration.remote_content {
                super::SshContent::RemoteHost(remote_host) => {
                    if self.is_http2()? {
                        return Ok(
                            HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                                location_id,
                                HttpProxyPassRemoteEndpoint::Http2OverSsh {
                                    ssh_credentials: ssh_configuration.credentials,
                                    remote_host,
                                },
                            ))
                            .into(),
                        );
                    } else {
                        return Ok(
                            HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                                location_id,
                                HttpProxyPassRemoteEndpoint::Http1OverSsh {
                                    ssh_credentials: ssh_configuration.credentials,
                                    remote_host,
                                },
                            ))
                            .into(),
                        );
                    }
                }
                super::SshContent::FilePath(file_path) => {
                    return Ok(HttpProxyPassContentSource::PathOverSsh(
                        PathOverSshContentSource::new(
                            ssh_configuration.credentials,
                            file_path,
                            self.default_file.clone(),
                            app.connection_settings.remote_connect_timeout,
                        ),
                    )
                    .into());
                }
            },
            ProxyPassTo::Tcp(_) => {
                return Ok(None);
            }
        }
    }

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
}
