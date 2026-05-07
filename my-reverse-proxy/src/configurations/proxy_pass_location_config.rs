use std::time::Duration;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{
    app::APP_CTX, http_content_source::local_path::LocalPathContentSrc, http_content_source::*,
    http_proxy_pass::content_source::*, settings::ModifyHttpHeadersSettings,
};

use super::*;

pub struct ProxyPassLocationConfig {
    pub path: String,
    pub id: i64,
    /// Logical identity of the (listen → upstream) pair, formatted as
    /// `"{listen_host}|{path}->{scheme}://{host}:{port}"`. On config apply
    /// we scan existing pools for a matching `id_string` and reuse the
    /// pool's `location_id` instead of allocating a fresh one — this keeps
    /// upstream pools warm across reloads.
    pub id_string: String,
    pub modify_request_headers: ModifyHeadersConfig,
    pub modify_response_headers: ModifyHeadersConfig,
    pub ip_white_list_id: Option<String>,
    pub domain_name: Option<String>,
    pub proxy_pass_to: ProxyPassToConfig,
    pub compress: bool,
    pub trace_payload: bool,
    pub auth_header: Option<String>,
}

impl ProxyPassLocationConfig {
    pub fn new(
        path: String,
        modify_headers: Option<ModifyHttpHeadersSettings>,
        ip_white_list_id: Option<String>,
        proxy_pass_to: ProxyPassToConfig,
        domain_name: Option<String>,
        compress: bool,
        trace_payload: bool,
        auth_header: Option<String>,
        listen_host: &str,
    ) -> Self {
        let mut modify_request_headers = ModifyHeadersConfig::default();
        let mut modify_response_headers = ModifyHeadersConfig::default();

        if let Some(mut modify_headers) = modify_headers {
            modify_request_headers.populate_request(&mut modify_headers);
            modify_response_headers.populate_response(&mut modify_headers);
        }

        let id_string = build_location_id_string(listen_host, &path, &proxy_pass_to);
        let id = crate::scripts::find_location_id_by_id_string(&id_string)
            .unwrap_or_else(|| APP_CTX.get_next_id());

        Self {
            path,
            id,
            id_string,
            modify_request_headers,
            modify_response_headers,
            ip_white_list_id,
            proxy_pass_to,
            domain_name,
            compress,
            trace_payload,
            auth_header,
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
            ProxyPassToConfig::Static(config) => HttpProxyPassContentSource::Static(
                crate::http_content_source::static_content::StaticContentSrc::new(config.clone()),
            ),
            ProxyPassToConfig::Http1(proxy_pass) | ProxyPassToConfig::McpHttp1(proxy_pass) => match &proxy_pass.remote_host {
                MyReverseProxyRemoteEndpoint::Gateway { .. } => {
                    todo!("Should not be here. Remove it");
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
                            let (pool_desc, pool_params, factory) =
                                make_tcp_h1_pool_factory(remote_host, debug, proxy_pass.connect_timeout, self.id, self.id_string.clone());
                            return HttpProxyPassContentSource::Http1(Http1ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                        rust_extensions::remote_endpoint::Scheme::Https => {
                            let (pool_desc, pool_params, factory) = make_tls_h1_pool_factory(
                                remote_host,
                                debug,
                                self.domain_name.clone(),
                                proxy_pass.connect_timeout,
                                self.id,
                                self.id_string.clone(),
                            );
                            return HttpProxyPassContentSource::Https1(Https1ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                        rust_extensions::remote_endpoint::Scheme::Ws => {
                            let (pool_desc, pool_params, factory) =
                                make_tcp_h1_pool_factory(remote_host, debug, proxy_pass.connect_timeout, self.id, self.id_string.clone());
                            return HttpProxyPassContentSource::Http1(Http1ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                        rust_extensions::remote_endpoint::Scheme::Wss => {
                            let (pool_desc, pool_params, factory) = make_tls_h1_pool_factory(
                                remote_host,
                                debug,
                                self.domain_name.clone(),
                                proxy_pass.connect_timeout,
                                self.id,
                                self.id_string.clone(),
                            );
                            return HttpProxyPassContentSource::Https1(Https1ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                        rust_extensions::remote_endpoint::Scheme::UnixSocket => {
                            let (pool_desc, pool_params, factory) =
                                make_uds_h1_pool_factory(remote_host, debug, proxy_pass.connect_timeout, self.id, self.id_string.clone());
                            return HttpProxyPassContentSource::UnixHttp1(UnixHttp1ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                    }
                }
            },

            ProxyPassToConfig::Http2(proxy_pass) => match &proxy_pass.remote_host {
                MyReverseProxyRemoteEndpoint::Gateway { .. } => {
                    todo!("Should not be here. Remote it at the end of the day");
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
                            let (pool_desc, pool_params, factory) =
                                make_tcp_h2_pool_factory(remote_host, debug, proxy_pass.connect_timeout, self.id, self.id_string.clone());
                            return HttpProxyPassContentSource::Http2(Http2ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                        rust_extensions::remote_endpoint::Scheme::Https => {
                            let (pool_desc, pool_params, factory) = make_tls_h2_pool_factory(
                                remote_host,
                                debug,
                                self.domain_name.clone(),
                                proxy_pass.connect_timeout,
                                self.id,
                                self.id_string.clone(),
                            );
                            return HttpProxyPassContentSource::Https2(Https2ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                        rust_extensions::remote_endpoint::Scheme::Ws => {
                            let (pool_desc, pool_params, factory) =
                                make_tcp_h1_pool_factory(remote_host, debug, proxy_pass.connect_timeout, self.id, self.id_string.clone());
                            return HttpProxyPassContentSource::Http1(Http1ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                        rust_extensions::remote_endpoint::Scheme::Wss => {
                            let (pool_desc, pool_params, factory) = make_tls_h1_pool_factory(
                                remote_host,
                                debug,
                                self.domain_name.clone(),
                                proxy_pass.connect_timeout,
                                self.id,
                                self.id_string.clone(),
                            );
                            return HttpProxyPassContentSource::Https1(Https1ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                        rust_extensions::remote_endpoint::Scheme::UnixSocket => {
                            let (pool_desc, pool_params, factory) =
                                make_uds_h2_pool_factory(remote_host, debug, proxy_pass.connect_timeout, self.id, self.id_string.clone());
                            return HttpProxyPassContentSource::UnixHttp2(UnixHttp2ContentSource {
                                pool_desc,
                                pool_params,
                                factory,
                                request_timeout: proxy_pass.request_timeout,
                            });
                        }
                    }
                }
            },
            ProxyPassToConfig::FilesPath(model) => match &model.files_path {
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
            ProxyPassToConfig::UnixHttp1(proxy_pass) => match &proxy_pass.remote_host {
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
                    let (pool_desc, pool_params, factory) =
                        make_uds_h1_pool_factory(remote_host, debug, proxy_pass.connect_timeout, self.id, self.id_string.clone());
                    return HttpProxyPassContentSource::UnixHttp1(UnixHttp1ContentSource {
                        pool_desc,
                        pool_params,
                        factory,
                        request_timeout: proxy_pass.request_timeout,
                    });
                }
            },
            ProxyPassToConfig::UnixHttp2(proxy_pass) => match &proxy_pass.remote_host {
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
                    let (pool_desc, pool_params, factory) =
                        make_uds_h2_pool_factory(remote_host, debug, proxy_pass.connect_timeout, self.id, self.id_string.clone());
                    return HttpProxyPassContentSource::UnixHttp2(UnixHttp2ContentSource {
                        pool_desc,
                        pool_params,
                        factory,
                        request_timeout: proxy_pass.request_timeout,
                    });
                }
            },
            ProxyPassToConfig::Drop => HttpProxyPassContentSource::Drop,
        };

        result
    }

    pub fn is_remote_content_http1(&self) -> Option<bool> {
        match &self.proxy_pass_to {
            ProxyPassToConfig::Http1(_) => Some(true),
            ProxyPassToConfig::McpHttp1(_) => Some(true),
            ProxyPassToConfig::UnixHttp1(_) => Some(true),
            ProxyPassToConfig::Http2(_) => Some(false),
            ProxyPassToConfig::UnixHttp2(_) => Some(false),
            _ => None,
        }
    }
}

fn build_location_id_string(
    listen_host: &str,
    path: &str,
    proxy_pass_to: &ProxyPassToConfig,
) -> String {
    format!(
        "{}|{}->{}|{}",
        listen_host,
        path,
        proxy_pass_to.get_type_as_str(),
        proxy_pass_to.to_string(),
    )
}

fn h1_tcp_pool_name(host: &str, port: u16, location_id: i64) -> String {
    format!("h1://{}:{}#{}", host, port, location_id)
}

fn h1_tls_pool_name(host: &str, port: u16, location_id: i64) -> String {
    format!("h1s://{}:{}#{}", host, port, location_id)
}

fn h1_uds_pool_name(socket_path: &str, location_id: i64) -> String {
    format!("uds-h1://{}#{}", socket_path, location_id)
}

fn h2_tcp_pool_name(host: &str, port: u16, location_id: i64) -> String {
    format!("h2://{}:{}#{}", host, port, location_id)
}

fn h2_tls_pool_name(host: &str, port: u16, location_id: i64) -> String {
    format!("h2s://{}:{}#{}", host, port, location_id)
}

fn h2_uds_pool_name(socket_path: &str, location_id: i64) -> String {
    format!("uds-h2://{}#{}", socket_path, location_id)
}

pub(crate) fn make_tcp_h1_pool_factory(
    remote_host: &std::sync::Arc<rust_extensions::remote_endpoint::RemoteEndpointOwned>,
    debug: bool,
    connect_timeout: Duration,
    location_id: i64,
    id_string: String,
) -> (
    crate::upstream_h1_pool::PoolDesc,
    crate::upstream_h1_pool::PoolParams,
    crate::upstream_h1_pool::ConnectorFactory<crate::http_client_connectors::HttpConnector>,
) {
    let port = remote_host.get_port().unwrap_or(80);
    let desc = crate::upstream_h1_pool::PoolDesc {
        location_id,
        name: h1_tcp_pool_name(remote_host.get_host(), port, location_id),
        id_string,
    };
    let endpoint_arc = remote_host.clone();
    let mut params = crate::upstream_h1_pool::PoolParams::default();
    params.connect_timeout = connect_timeout;
    let factory: crate::upstream_h1_pool::ConnectorFactory<
        crate::http_client_connectors::HttpConnector,
    > = std::sync::Arc::new(move || {
        let metrics: std::sync::Arc<
            dyn my_http_client::http1::MyHttpClientMetrics + Send + Sync + 'static,
        > = APP_CTX.prometheus.clone();
        (
            crate::http_client_connectors::HttpConnector {
                remote_endpoint: endpoint_arc.clone(),
                debug,
            },
            metrics,
        )
    });
    (desc, params, factory)
}

pub(crate) fn make_tls_h1_pool_factory(
    remote_host: &std::sync::Arc<rust_extensions::remote_endpoint::RemoteEndpointOwned>,
    debug: bool,
    domain_name: Option<String>,
    connect_timeout: Duration,
    location_id: i64,
    id_string: String,
) -> (
    crate::upstream_h1_pool::PoolDesc,
    crate::upstream_h1_pool::PoolParams,
    crate::upstream_h1_pool::ConnectorFactory<crate::http_client_connectors::HttpTlsConnector>,
) {
    let port = remote_host.get_port().unwrap_or(443);
    let desc = crate::upstream_h1_pool::PoolDesc {
        location_id,
        name: h1_tls_pool_name(remote_host.get_host(), port, location_id),
        id_string,
    };
    let endpoint_arc = remote_host.clone();
    let mut params = crate::upstream_h1_pool::PoolParams::default();
    params.connect_timeout = connect_timeout;
    let factory: crate::upstream_h1_pool::ConnectorFactory<
        crate::http_client_connectors::HttpTlsConnector,
    > = std::sync::Arc::new(move || {
        let metrics: std::sync::Arc<
            dyn my_http_client::http1::MyHttpClientMetrics + Send + Sync + 'static,
        > = APP_CTX.prometheus.clone();
        (
            crate::http_client_connectors::HttpTlsConnector {
                remote_endpoint: endpoint_arc.clone(),
                domain_name: domain_name.clone(),
                debug,
            },
            metrics,
        )
    });
    (desc, params, factory)
}

pub(crate) fn make_uds_h1_pool_factory(
    remote_host: &std::sync::Arc<rust_extensions::remote_endpoint::RemoteEndpointOwned>,
    debug: bool,
    connect_timeout: Duration,
    location_id: i64,
    id_string: String,
) -> (
    crate::upstream_h1_pool::PoolDesc,
    crate::upstream_h1_pool::PoolParams,
    crate::upstream_h1_pool::ConnectorFactory<crate::http_client_connectors::UnixSocketHttpConnector>,
) {
    let desc = crate::upstream_h1_pool::PoolDesc {
        location_id,
        name: h1_uds_pool_name(remote_host.get_host_port().as_str(), location_id),
        id_string,
    };
    let endpoint_arc = remote_host.clone();
    let mut params = crate::upstream_h1_pool::PoolParams::default();
    params.connect_timeout = connect_timeout;
    let factory: crate::upstream_h1_pool::ConnectorFactory<
        crate::http_client_connectors::UnixSocketHttpConnector,
    > = std::sync::Arc::new(move || {
        let metrics: std::sync::Arc<
            dyn my_http_client::http1::MyHttpClientMetrics + Send + Sync + 'static,
        > = APP_CTX.prometheus.clone();
        (
            crate::http_client_connectors::UnixSocketHttpConnector {
                remote_endpoint: endpoint_arc.clone(),
                debug,
            },
            metrics,
        )
    });
    (desc, params, factory)
}

pub(crate) fn make_tcp_h2_pool_factory(
    remote_host: &std::sync::Arc<rust_extensions::remote_endpoint::RemoteEndpointOwned>,
    debug: bool,
    connect_timeout: Duration,
    location_id: i64,
    id_string: String,
) -> (
    crate::upstream_h2_pool::PoolDesc,
    crate::upstream_h2_pool::PoolParams,
    crate::upstream_h2_pool::ConnectorFactory<crate::http_client_connectors::HttpConnector>,
) {
    let port = remote_host.get_port().unwrap_or(80);
    let host = remote_host.get_host();
    let desc = crate::upstream_h2_pool::PoolDesc {
        location_id,
        name: h2_tcp_pool_name(host, port, location_id),
        authority: format!("{}:{}", host, port),
        id_string,
    };
    let endpoint_arc = remote_host.clone();
    let mut params = crate::upstream_h2_pool::PoolParams::default();
    params.connect_timeout = connect_timeout;
    params.health_check_path = APP_CTX.default_h2_livness_url.clone();
    let factory: crate::upstream_h2_pool::ConnectorFactory<
        crate::http_client_connectors::HttpConnector,
    > = std::sync::Arc::new(move || {
        let metrics: std::sync::Arc<
            dyn my_http_client::hyper::MyHttpHyperClientMetrics + Send + Sync + 'static,
        > = APP_CTX.prometheus.clone();
        (
            crate::http_client_connectors::HttpConnector {
                remote_endpoint: endpoint_arc.clone(),
                debug,
            },
            metrics,
        )
    });
    (desc, params, factory)
}

pub(crate) fn make_tls_h2_pool_factory(
    remote_host: &std::sync::Arc<rust_extensions::remote_endpoint::RemoteEndpointOwned>,
    debug: bool,
    domain_name: Option<String>,
    connect_timeout: Duration,
    location_id: i64,
    id_string: String,
) -> (
    crate::upstream_h2_pool::PoolDesc,
    crate::upstream_h2_pool::PoolParams,
    crate::upstream_h2_pool::ConnectorFactory<crate::http_client_connectors::HttpTlsConnector>,
) {
    let port = remote_host.get_port().unwrap_or(443);
    let host = remote_host.get_host();
    let desc = crate::upstream_h2_pool::PoolDesc {
        location_id,
        name: h2_tls_pool_name(host, port, location_id),
        authority: format!("{}:{}", host, port),
        id_string,
    };
    let endpoint_arc = remote_host.clone();
    let mut params = crate::upstream_h2_pool::PoolParams::default();
    params.connect_timeout = connect_timeout;
    params.health_check_path = APP_CTX.default_h2_livness_url.clone();
    let factory: crate::upstream_h2_pool::ConnectorFactory<
        crate::http_client_connectors::HttpTlsConnector,
    > = std::sync::Arc::new(move || {
        let metrics: std::sync::Arc<
            dyn my_http_client::hyper::MyHttpHyperClientMetrics + Send + Sync + 'static,
        > = APP_CTX.prometheus.clone();
        (
            crate::http_client_connectors::HttpTlsConnector {
                remote_endpoint: endpoint_arc.clone(),
                domain_name: domain_name.clone(),
                debug,
            },
            metrics,
        )
    });
    (desc, params, factory)
}

pub(crate) fn make_uds_h2_pool_factory(
    remote_host: &std::sync::Arc<rust_extensions::remote_endpoint::RemoteEndpointOwned>,
    debug: bool,
    connect_timeout: Duration,
    location_id: i64,
    id_string: String,
) -> (
    crate::upstream_h2_pool::PoolDesc,
    crate::upstream_h2_pool::PoolParams,
    crate::upstream_h2_pool::ConnectorFactory<crate::http_client_connectors::UnixSocketHttpConnector>,
) {
    let desc = crate::upstream_h2_pool::PoolDesc {
        location_id,
        name: h2_uds_pool_name(remote_host.get_host_port().as_str(), location_id),
        authority: "localhost".to_string(),
        id_string,
    };
    let endpoint_arc = remote_host.clone();
    let mut params = crate::upstream_h2_pool::PoolParams::default();
    params.connect_timeout = connect_timeout;
    params.health_check_path = APP_CTX.default_h2_livness_url.clone();
    let factory: crate::upstream_h2_pool::ConnectorFactory<
        crate::http_client_connectors::UnixSocketHttpConnector,
    > = std::sync::Arc::new(move || {
        let metrics: std::sync::Arc<
            dyn my_http_client::hyper::MyHttpHyperClientMetrics + Send + Sync + 'static,
        > = APP_CTX.prometheus.clone();
        (
            crate::http_client_connectors::UnixSocketHttpConnector {
                remote_endpoint: endpoint_arc.clone(),
                debug,
            },
            metrics,
        )
    });
    (desc, params, factory)
}
