use std::time::Duration;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{
    app::APP_CTX,
    http_content_source::*,
    http_proxy_pass::content_source::*,
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
    pub trace_payload: bool,
}

impl ProxyPassLocationConfig {
    pub fn new(
        path: String,
        modify_headers: Option<ModifyHttpHeadersSettings>,
        ip_white_list_id: Option<String>,
        proxy_pass_to: ProxyPassTo,
        domain_name: Option<String>,
        compress: bool,
        trace_payload: bool,
    ) -> Self {
        //println!("Created location to {:?}", proxy_pass_to);

        Self {
            path,
            id: APP_CTX.get_next_id(),
            modify_headers,
            ip_white_list_id,
            proxy_pass_to,
            domain_name,
            compress,
            trace_payload,
        }
    }
    pub fn get_proxy_pass_to_as_string(&self) -> String {
        self.proxy_pass_to.to_string()
    }

    pub async fn create_data_source(
        &self,
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
            ProxyPassTo::Http1(proxy_pass) => match &proxy_pass.remote_host {
                MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                    let model = Http1OverGatewayContentSource {
                        gateway_id: id.clone(),
                        remote_endpoint: remote_host.clone(),
                    };
                    return HttpProxyPassContentSource::Http1OverGateway(model);
                }
                MyReverseProxyRemoteEndpoint::OverSsh {
                    ssh_credentials,
                    remote_host,
                } => {
                    let ssh_session = crate::scripts::ssh::get_ssh_session(&ssh_credentials)
                        .await
                        .unwrap();

                    let model = Http1OverSshContentSource {
                        over_ssh: OverSshConnectionSettings {
                            ssh_credentials: ssh_credentials.clone().into(),
                            remote_resource_string: remote_host.as_str().to_string(),
                        },
                        ssh_session,
                        debug,
                        request_timeout: proxy_pass.request_timeout,
                        connect_timeout: proxy_pass.connect_timeout,
                    };

                    HttpProxyPassContentSource::Http1OverSsh(model)
                }
                MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    let remote_endpoint_scheme = remote_host.get_scheme();

                    if remote_endpoint_scheme.is_none() {
                        panic!(
                            "Scheme is not set for remote resource {}",
                            remote_host.as_str()
                        );
                    }

                    match remote_endpoint_scheme.as_ref().unwrap() {
                        rust_extensions::remote_endpoint::Scheme::Http => {
                            let model = Http1ContentSource {
                                remote_endpoint: remote_host.to_owned(),
                                debug,
                                request_timeout: proxy_pass.request_timeout,
                                connect_timeout: proxy_pass.connect_timeout,
                            };
                            return HttpProxyPassContentSource::Http1(model);
                        }
                        rust_extensions::remote_endpoint::Scheme::Https => {
                            let model = Https1ContentSource {
                                remote_endpoint: remote_host.to_owned(),
                                domain_name: self.domain_name.clone(),
                                debug,
                                request_timeout: proxy_pass.request_timeout,
                                connect_timeout: proxy_pass.connect_timeout,
                            };
                            return HttpProxyPassContentSource::Https1(model);
                        }
                        rust_extensions::remote_endpoint::Scheme::Ws => {
                            let model = Http1ContentSource {
                                remote_endpoint: remote_host.to_owned(),
                                debug,
                                request_timeout: proxy_pass.request_timeout,
                                connect_timeout: proxy_pass.connect_timeout,
                            };
                            return HttpProxyPassContentSource::Http1(model);
                        }
                        rust_extensions::remote_endpoint::Scheme::Wss => {
                            let model = Https1ContentSource {
                                remote_endpoint: remote_host.to_owned(),
                                domain_name: self.domain_name.clone(),
                                debug,
                                request_timeout: proxy_pass.request_timeout,
                                connect_timeout: proxy_pass.connect_timeout,
                            };
                            return HttpProxyPassContentSource::Https1(model);
                        }
                        rust_extensions::remote_endpoint::Scheme::UnixSocket => {
                            panic!("HTTP1 UnixSocket is not supported as remote content source");
                        }
                    }
                }
            },
            ProxyPassTo::Http2(proxy_pass) => match &proxy_pass.remote_host {
                MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                    let model = Http2OverGatewayContentSource {
                        gateway_id: id.clone(),
                        remote_endpoint: remote_host.clone(),
                    };
                    return HttpProxyPassContentSource::Http2OverGateway(model);
                }
                MyReverseProxyRemoteEndpoint::OverSsh {
                    ssh_credentials,
                    remote_host,
                } => {
                    let ssh_session = crate::scripts::ssh::get_ssh_session(ssh_credentials)
                        .await
                        .unwrap();

                    let model = Http2OverSshContentSource {
                        over_ssh: OverSshConnectionSettings {
                            ssh_credentials: ssh_credentials.clone().into(),
                            remote_resource_string: remote_host.as_str().to_string(),
                        },
                        ssh_session: ssh_session.clone(),
                        debug,
                        request_timeout: proxy_pass.request_timeout,
                        connect_timeout: proxy_pass.connect_timeout,
                    };

                    return HttpProxyPassContentSource::Http2OverSsh(model);
                }
                MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    let remote_endpoint_scheme = remote_host.get_scheme();

                    if remote_endpoint_scheme.is_none() {
                        panic!(
                            "Scheme is not set for remote resource {}",
                            remote_host.as_str()
                        );
                    }

                    match remote_endpoint_scheme.as_ref().unwrap() {
                        rust_extensions::remote_endpoint::Scheme::Http => {
                            let model = Http2ContentSource {
                                remote_endpoint: remote_host.to_owned(),
                                debug,
                                request_timeout: proxy_pass.request_timeout,
                                connect_timeout: proxy_pass.connect_timeout,
                            };

                            return HttpProxyPassContentSource::Http2(model);
                        }
                        rust_extensions::remote_endpoint::Scheme::Https => {
                            let model = Https2ContentSource {
                                remote_endpoint: remote_host.to_owned(),
                                domain_name: self.domain_name.clone(),
                                debug,
                                request_timeout: proxy_pass.request_timeout,
                                connect_timeout: proxy_pass.connect_timeout,
                            };
                            return HttpProxyPassContentSource::Https2(model);
                        }
                        rust_extensions::remote_endpoint::Scheme::Ws => {
                            let model = Http1ContentSource {
                                remote_endpoint: remote_host.to_owned(),
                                debug,
                                request_timeout: proxy_pass.request_timeout,
                                connect_timeout: proxy_pass.connect_timeout,
                            };
                            return HttpProxyPassContentSource::Http1(model);
                        }
                        rust_extensions::remote_endpoint::Scheme::Wss => {
                            let model = Https1ContentSource {
                                remote_endpoint: remote_host.to_owned(),
                                domain_name: self.domain_name.clone(),
                                debug,
                                request_timeout: proxy_pass.request_timeout,
                                connect_timeout: proxy_pass.connect_timeout,
                            };
                            return HttpProxyPassContentSource::Https1(model);
                        }
                        rust_extensions::remote_endpoint::Scheme::UnixSocket => {
                            panic!("HTTP2 UnixSocket is not supported as remote content source");
                        }
                    }
                }
            },
            ProxyPassTo::FilesPath(model) => match &model.files_path {
                MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                    let model = PathOverGatewayContentSource {
                        gateway_id: id.clone(),
                        path: remote_host.clone(),
                        default_file: model.default_file.clone(),
                    };
                    HttpProxyPassContentSource::PathOverGateway(model)
                }
                MyReverseProxyRemoteEndpoint::OverSsh {
                    ssh_credentials,
                    remote_host,
                } => {
                    let ssh_session = crate::scripts::ssh::get_ssh_session(ssh_credentials)
                        .await
                        .unwrap();
                    let src = PathOverSshContentSource::new(
                        ssh_session,
                        remote_host.as_str().to_string(),
                        model.default_file.clone(),
                        timeout,
                    );

                    HttpProxyPassContentSource::PathOverSsh(src)
                }
                MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    let local_file_path = LocalFilePath::new(remote_host.as_str().to_string());
                    HttpProxyPassContentSource::LocalPath(LocalPathContentSrc::new(
                        &local_file_path,
                        model.default_file.clone(),
                    ))
                }
            },
            ProxyPassTo::UnixHttp1(proxy_pass) => match &proxy_pass.remote_host {
                MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                    panic!(
                        "Unix+Http is not supported  over gateway. Id: {}. RemoteHost: {}",
                        id.as_str(),
                        remote_host.as_str()
                    );
                }
                MyReverseProxyRemoteEndpoint::OverSsh {
                    ssh_credentials,
                    remote_host,
                } => {
                    panic!(
                        "Unix+Http is not supported  over ssh. host_port: {}:{}. Remote_host: {}",
                        ssh_credentials.get_host_port().0,
                        ssh_credentials.get_host_port().1,
                        remote_host.as_str()
                    );
                }
                MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    let model = UnixHttp1ContentSource {
                        remote_endpoint: remote_host.to_owned(),
                        debug,
                        request_timeout: proxy_pass.request_timeout,
                        connect_timeout: proxy_pass.connect_timeout,
                        connection_id: self.id,
                    };
                    return HttpProxyPassContentSource::UnixHttp1(model);
                }
            },
            ProxyPassTo::UnixHttp2(proxy_pass) => match &proxy_pass.remote_host {
                MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                    panic!(
                        "Unix+Http2 is not supported  over gateway. Id:{}. RemoteHost: {}",
                        id.as_str(),
                        remote_host.as_str()
                    );
                }
                MyReverseProxyRemoteEndpoint::OverSsh {
                    ssh_credentials,
                    remote_host,
                } => {
                    panic!(
                        "Unix+Http2 is not supported  over ssh. HostPort: {}. RemoteHost: {}",
                        ssh_credentials.get_host_port_as_string(),
                        remote_host.as_str()
                    );
                }
                MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    let model = UnixHttp2ContentSource {
                        remote_endpoint: remote_host.to_owned(),
                        debug,
                        request_timeout: proxy_pass.request_timeout,
                        connect_timeout: proxy_pass.connect_timeout,
                        connection_id: self.id,
                    };
                    return HttpProxyPassContentSource::UnixHttp2(model);
                }
            },
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
