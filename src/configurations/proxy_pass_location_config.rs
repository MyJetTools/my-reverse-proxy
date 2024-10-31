use std::time::Duration;

use crate::{
    http_content_source::{
        LocalPathContentSrc, PathOverSshContentSource, RemoteHttpContentSource, StaticContentSrc,
    },
    http_proxy_pass::{HttpProxyPassContentSource, HttpProxyPassRemoteEndpoint},
    settings::{ModifyHttpHeadersSettings, ProxyPassTo},
    types::WhiteListedIpList,
};

use super::*;

pub struct ProxyPassLocationConfig {
    pub path: String,
    pub id: i64,
    pub modify_headers: Option<ModifyHttpHeadersSettings>,
    pub whitelisted_ip: WhiteListedIpList,
    pub remote_type: HttpType,
    pub domain_name: Option<String>,
    proxy_pass_to: ProxyPassTo,
    pub compress: bool,
}

impl ProxyPassLocationConfig {
    pub fn new(
        id: i64,
        path: String,
        modify_headers: Option<ModifyHttpHeadersSettings>,
        whitelisted_ip: WhiteListedIpList,
        proxy_pass_to: ProxyPassTo,
        domain_name: Option<String>,
        remote_type: HttpType,
        compress: bool,
    ) -> Self {
        Self {
            path,
            id,
            modify_headers,
            whitelisted_ip,
            proxy_pass_to,
            remote_type,
            domain_name,
            compress,
        }
    }
    pub fn get_proxy_pass_to_as_string(&self) -> String {
        self.proxy_pass_to.to_string()
    }

    /*
    pub fn is_my_uri(&self, uri: &Uri) -> bool {
        let result = rust_extensions::str_utils::starts_with_case_insensitive(
            uri.path(),
            self.path.as_str(),
        );

        result
    }
     */

    pub fn create_content_source(
        &self,
        debug: bool,
        timeout: Duration,
    ) -> HttpProxyPassContentSource {
        match &self.proxy_pass_to {
            ProxyPassTo::Static(static_content_model) => {
                HttpProxyPassContentSource::Static(StaticContentSrc::new(
                    static_content_model.status_code,
                    static_content_model.content_type.clone(),
                    static_content_model.body.clone(),
                ))
            }
            ProxyPassTo::Http(remote_host) => {
                HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                    self.id,
                    HttpProxyPassRemoteEndpoint::Http(RemoteHost::new(remote_host.to_string())),
                    debug,
                ))
            }

            ProxyPassTo::Http2(remote_host) => {
                HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                    self.id,
                    HttpProxyPassRemoteEndpoint::Http2(RemoteHost::new(remote_host.to_string())),
                    debug,
                ))
            }
            ProxyPassTo::LocalPath(model) => HttpProxyPassContentSource::LocalPath(
                LocalPathContentSrc::new(&model.local_path, model.default_file.clone()),
            ),
            ProxyPassTo::Ssh(model) => match &model.ssh_config.remote_content {
                SshContent::RemoteHost(remote_host) => {
                    if model.http2 {
                        HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                            self.id,
                            HttpProxyPassRemoteEndpoint::Http2OverSsh {
                                ssh_credentials: model.ssh_config.credentials.clone(),
                                remote_host: remote_host.clone(),
                            },
                            debug,
                        ))
                    } else {
                        HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                            self.id,
                            HttpProxyPassRemoteEndpoint::Http1OverSsh {
                                ssh_credentials: model.ssh_config.credentials.clone(),
                                remote_host: remote_host.clone(),
                            },
                            debug,
                        ))
                    }
                }
                SshContent::FilePath(file_path) => {
                    HttpProxyPassContentSource::PathOverSsh(PathOverSshContentSource::new(
                        model.ssh_config.credentials.clone(),
                        file_path.clone(),
                        model.default_file.clone(),
                        timeout,
                    ))
                }
            },
            ProxyPassTo::Tcp(_) => {
                panic!("Should not be here.")
            }
        }
    }
}
