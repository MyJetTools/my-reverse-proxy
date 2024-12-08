use std::time::Duration;

use crate::{
    app::AppContext,
    http_client::{Http1Client, Http2Client, SshConnector},
    http_content_source::{LocalPathContentSrc, PathOverSshContentSource, StaticContentSrc},
    http_proxy_pass::HttpProxyPassContentSource,
    settings::{ModifyHttpHeadersSettings, ProxyPassTo},
    types::WhiteListedIpList,
};

use super::*;
use my_http_client::{http1::MyHttpClient, http2::MyHttp2Client};

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

    pub fn create_data_source(
        &self,
        app: &AppContext,
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
            ProxyPassTo::Http1(remote_host) => {
                let http_client = Http1Client::create(
                    app.prometheus.clone(),
                    remote_host.clone(),
                    self.domain_name.clone(),
                    debug,
                );
                HttpProxyPassContentSource::Http1(http_client)
            }

            ProxyPassTo::Http2(remote_host) => {
                let http_client =
                    Http2Client::create(app, remote_host.clone(), self.domain_name.clone(), debug);
                HttpProxyPassContentSource::Http2(http_client)
            }
            ProxyPassTo::LocalPath(model) => HttpProxyPassContentSource::LocalPath(
                LocalPathContentSrc::new(&model.local_path, model.default_file.clone()),
            ),
            ProxyPassTo::Ssh(model) => match &model.ssh_config.remote_content {
                SshContent::RemoteHost(remote_host) => {
                    if model.http2 {
                        let connector = SshConnector {
                            use_connection_pool: true,
                            ssh_credentials: model.ssh_config.credentials.clone(),
                            remote_host: remote_host.clone(),
                            debug,
                        };

                        let http_client = MyHttp2Client::new(connector, app.prometheus.clone());

                        HttpProxyPassContentSource::Http2OverSsh(http_client)
                    } else {
                        let connector = SshConnector {
                            use_connection_pool: true,
                            ssh_credentials: model.ssh_config.credentials.clone(),
                            remote_host: remote_host.clone(),
                            debug,
                        };

                        let http_client =
                            MyHttpClient::new_with_metrics(connector, app.prometheus.clone());

                        HttpProxyPassContentSource::Http1OverSsh(http_client)
                    }
                }
                SshContent::FilePath(file_path) => {
                    let src = PathOverSshContentSource::new(
                        true,
                        model.ssh_config.credentials.clone(),
                        file_path.clone(),
                        model.default_file.clone(),
                        timeout,
                    );

                    HttpProxyPassContentSource::PathOverSsh(src)
                }
            },
            ProxyPassTo::Tcp(_) => {
                panic!("Should not be here.")
            }
        };

        result
    }

    pub fn is_http1(&self) -> Option<bool> {
        match &self.proxy_pass_to {
            ProxyPassTo::Http1(_) => Some(true),
            ProxyPassTo::Http2(_) => Some(false),
            ProxyPassTo::Ssh(model) => match &model.ssh_config.remote_content {
                SshContent::RemoteHost(_) => Some(!model.http2),
                SshContent::FilePath(_) => None,
            },
            _ => None,
        }
    }
}
