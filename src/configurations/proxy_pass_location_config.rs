use std::{sync::Arc, time::Duration};

use crate::{
    app::AppContext,
    http_content_source::{LocalPathContentSrc, PathOverSshContentSource, StaticContentSrc},
    http_proxy_pass::HttpProxyPassContentSource,
    settings::{ModifyHttpHeadersSettings, ProxyPassTo},
};

use super::*;

pub struct ProxyPassLocationConfig {
    pub path: String,
    pub id: i64,
    pub modify_headers: Option<ModifyHttpHeadersSettings>,
    pub ip_white_list_id: Option<String>,
    pub domain_name: Option<String>,
    pub proxy_pass_to: ProxyPassTo,
    pub compress: bool,
}

impl ProxyPassLocationConfig {
    pub fn new(
        id: i64,
        path: String,
        modify_headers: Option<ModifyHttpHeadersSettings>,
        ip_white_list_id: Option<String>,
        proxy_pass_to: ProxyPassTo,
        domain_name: Option<String>,
        compress: bool,
    ) -> Self {
        Self {
            path,
            id,
            modify_headers,
            ip_white_list_id,
            proxy_pass_to,
            domain_name,
            compress,
        }
    }
    pub fn get_proxy_pass_to_as_string(&self) -> String {
        self.proxy_pass_to.to_string()
    }

    pub async fn create_data_source(
        &self,
        app: &Arc<AppContext>,
        debug: bool,
        timeout: Duration,
    ) -> HttpProxyPassContentSource {
        let result = match &self.proxy_pass_to {
            ProxyPassTo::Static(static_content_model) => {
                HttpProxyPassContentSource::Static(StaticContentSrc::new(
                    static_content_model.status_code,
                    static_content_model.content_type.clone(),
                    static_content_model.body.clone(),
                ))
            }
            ProxyPassTo::Http1(remote_content) => {
                if let Some(ssh_credentials) = remote_content.ssh_credentials.as_ref() {
                    let ssh_session = crate::scripts::ssh::get_ssh_session(app, ssh_credentials)
                        .await
                        .unwrap();

                    HttpProxyPassContentSource::Http1OverSsh {
                        app: app.clone(),
                        over_ssh: remote_content.clone(),
                        ssh_session: ssh_session.clone(),
                        debug,
                    }
                } else {
                    let remote_endpoint = remote_content.get_remote_endpoint();

                    let remote_endpoint_scheme = remote_endpoint.get_scheme();

                    if remote_endpoint_scheme.is_none() {
                        panic!(
                            "Scheme is not set for remote resource {}",
                            remote_endpoint.as_str()
                        );
                    }

                    match remote_endpoint_scheme.as_ref().unwrap() {
                        rust_extensions::remote_endpoint::Scheme::Http => {
                            HttpProxyPassContentSource::Http1 {
                                app: app.clone(),
                                remote_endpoint: remote_endpoint.to_owned(),
                                debug,
                            }
                        }
                        rust_extensions::remote_endpoint::Scheme::Https => {
                            HttpProxyPassContentSource::Https1 {
                                app: app.clone(),
                                remote_endpoint: remote_endpoint.to_owned(),
                                domain_name: self.domain_name.clone(),
                                debug,
                            }
                        }
                        rust_extensions::remote_endpoint::Scheme::Ws => {
                            HttpProxyPassContentSource::Http1 {
                                app: app.clone(),
                                remote_endpoint: remote_endpoint.to_owned(),
                                debug,
                            }
                        }
                        rust_extensions::remote_endpoint::Scheme::Wss => {
                            HttpProxyPassContentSource::Https1 {
                                app: app.clone(),
                                remote_endpoint: remote_endpoint.to_owned(),
                                domain_name: self.domain_name.clone(),
                                debug,
                            }
                        }
                        rust_extensions::remote_endpoint::Scheme::UnixSocket => {
                            panic!("HTTP1 UnixSocket is not supported as remote content source");
                        }
                    }
                }
            }

            ProxyPassTo::Http2(remote_host) => {
                if let Some(ssh_credentials) = remote_host.ssh_credentials.as_ref() {
                    let ssh_session = crate::scripts::ssh::get_ssh_session(app, ssh_credentials)
                        .await
                        .unwrap();

                    HttpProxyPassContentSource::Http2OverSsh {
                        app: app.clone(),
                        over_ssh: remote_host.clone(),
                        ssh_session: ssh_session.clone(),
                        debug,
                    }
                } else {
                    let remote_endpoint = remote_host.get_remote_endpoint();

                    let remote_endpoint_scheme = remote_endpoint.get_scheme();

                    if remote_endpoint_scheme.is_none() {
                        panic!(
                            "Scheme is not set for remote resource {}",
                            remote_endpoint.as_str()
                        );
                    }

                    match remote_endpoint_scheme.as_ref().unwrap() {
                        rust_extensions::remote_endpoint::Scheme::Http => {
                            HttpProxyPassContentSource::Http2 {
                                app: app.clone(),
                                remote_endpoint: remote_endpoint.to_owned(),
                                debug,
                            }
                        }
                        rust_extensions::remote_endpoint::Scheme::Https => {
                            HttpProxyPassContentSource::Https2 {
                                app: app.clone(),
                                remote_endpoint: remote_endpoint.to_owned(),
                                domain_name: self.domain_name.clone(),
                                debug,
                            }
                        }
                        rust_extensions::remote_endpoint::Scheme::Ws => {
                            HttpProxyPassContentSource::Http2 {
                                app: app.clone(),
                                remote_endpoint: remote_endpoint.to_owned(),
                                debug,
                            }
                        }
                        rust_extensions::remote_endpoint::Scheme::Wss => {
                            HttpProxyPassContentSource::Https2 {
                                app: app.clone(),
                                remote_endpoint: remote_endpoint.to_owned(),
                                domain_name: self.domain_name.clone(),
                                debug,
                            }
                        }
                        rust_extensions::remote_endpoint::Scheme::UnixSocket => {
                            panic!("HTTP2 UnixSocket is not supported as remote content source");
                        }
                    }
                }
            }
            ProxyPassTo::FilesPath(model) => {
                if let Some(ssh_credentials) = model.files_path.ssh_credentials.as_ref() {
                    let ssh_session = crate::scripts::ssh::get_ssh_session(app, ssh_credentials)
                        .await
                        .unwrap();
                    let src = PathOverSshContentSource::new(
                        ssh_session,
                        model.files_path.remote_resource_string.to_string(),
                        model.default_file.clone(),
                        timeout,
                    );

                    return HttpProxyPassContentSource::PathOverSsh(src);
                }

                let local_file_path =
                    LocalFilePath::new(model.files_path.remote_resource_string.to_string());
                HttpProxyPassContentSource::LocalPath(LocalPathContentSrc::new(
                    &local_file_path,
                    model.default_file.clone(),
                ))
            }
        };

        result
    }

    pub fn is_remote_content_http1(&self) -> Option<bool> {
        match &self.proxy_pass_to {
            ProxyPassTo::Http1(_) => Some(true),
            ProxyPassTo::Http2(_) => Some(false),
            _ => None,
        }
    }
}
