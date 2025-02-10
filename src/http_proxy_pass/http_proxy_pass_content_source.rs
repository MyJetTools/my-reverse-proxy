use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use my_ssh::{ssh_settings::OverSshConnectionSettings, SshAsyncChannel, SshSession};
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{
    app::AppContext,
    http_client_connectors::{HttpConnector, HttpOverSshConnector, HttpTlsConnector},
    http_content_source::{LocalPathContentSrc, PathOverSshContentSource, StaticContentSrc},
};

use super::ProxyPassError;
use my_http_client::{http1::*, MyHttpClientDisconnect};

pub enum HttpProxyPassContentSource {
    Http1 {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Https1 {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        domain_name: Option<String>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Http2 {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Https2 {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        domain_name: Option<String>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Http1OverSsh {
        app: Arc<AppContext>,
        over_ssh: OverSshConnectionSettings,
        ssh_session: Arc<SshSession>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Http2OverSsh {
        app: Arc<AppContext>,
        over_ssh: OverSshConnectionSettings,
        ssh_session: Arc<SshSession>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    LocalPath(LocalPathContentSrc),
    PathOverSsh(PathOverSshContentSource),
    PathOverGateway {
        gateway_id: Arc<String>,
        path: Arc<RemoteEndpointOwned>,
        default_file: Option<String>,
    },
    Static(StaticContentSrc),
}

impl HttpProxyPassContentSource {
    pub async fn send_request(
        &self,
        app: &Arc<AppContext>,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        match self {
            HttpProxyPassContentSource::Http1 {
                app,
                remote_endpoint,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let mut http_client = app
                    .http_clients_pool
                    .get(remote_endpoint.as_str().into(), *connect_timeout, || {
                        HttpConnector {
                            remote_endpoint: remote_endpoint.clone(),
                            debug: *debug,
                        }
                    })
                    .await;

                let req = MyHttpRequest::from_hyper_request(req).await;

                match http_client.do_request(&req, *request_timeout).await? {
                    MyHttpResponse::Response(response) => {
                        return Ok(HttpResponse::Response(response));
                    }
                    MyHttpResponse::WebSocketUpgrade {
                        stream,
                        response,
                        disconnection,
                    } => {
                        http_client.upgraded_to_websocket();
                        return Ok(HttpResponse::WebSocketUpgrade {
                            stream: WebSocketUpgradeStream::TcpStream(stream),
                            response,
                            disconnection,
                        });
                    }
                }
            }

            HttpProxyPassContentSource::Https1 {
                app,
                remote_endpoint,
                domain_name,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let mut http_client = app
                    .https_clients_pool
                    .get(remote_endpoint.as_str().into(), *connect_timeout, || {
                        HttpTlsConnector {
                            remote_endpoint: remote_endpoint.clone(),
                            debug: *debug,
                            domain_name: domain_name.clone(),
                        }
                    })
                    .await;

                let req = MyHttpRequest::from_hyper_request(req).await;

                match http_client.do_request(&req, *request_timeout).await? {
                    MyHttpResponse::Response(response) => {
                        return Ok(HttpResponse::Response(response));
                    }
                    MyHttpResponse::WebSocketUpgrade {
                        stream,
                        response,
                        disconnection,
                    } => {
                        http_client.upgraded_to_websocket();
                        return Ok(HttpResponse::WebSocketUpgrade {
                            stream: WebSocketUpgradeStream::TlsStream(stream),
                            response,
                            disconnection,
                        });
                    }
                }
            }

            HttpProxyPassContentSource::Http2 {
                app,
                remote_endpoint,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let http_client = app
                    .http2_clients_pool
                    .get(remote_endpoint.as_str().into(), *connect_timeout, || {
                        (
                            HttpConnector {
                                remote_endpoint: remote_endpoint.clone(),
                                debug: *debug,
                            },
                            app.prometheus.clone(),
                        )
                    })
                    .await;

                let response = http_client.do_request(req, *request_timeout).await?;
                return Ok(HttpResponse::Response(response));
            }
            HttpProxyPassContentSource::Https2 {
                app,
                remote_endpoint,
                domain_name,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let http_client = app
                    .https2_clients_pool
                    .get(remote_endpoint.as_str().into(), *connect_timeout, || {
                        (
                            HttpTlsConnector {
                                remote_endpoint: remote_endpoint.clone(),
                                debug: *debug,
                                domain_name: domain_name.clone(),
                            },
                            app.prometheus.clone(),
                        )
                    })
                    .await;

                let response = http_client.do_request(req, *request_timeout).await?;
                return Ok(HttpResponse::Response(response));
            }
            HttpProxyPassContentSource::Http1OverSsh {
                app,
                over_ssh,
                ssh_session,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let mut http_client = app
                    .http_over_ssh_clients_pool
                    .get(over_ssh.to_string().into(), *connect_timeout, || {
                        HttpOverSshConnector {
                            remote_endpoint: over_ssh.get_remote_endpoint().to_owned(),
                            debug: *debug,
                            ssh_session: ssh_session.clone(),
                            connect_timeout: *connect_timeout,
                        }
                    })
                    .await;

                let req = MyHttpRequest::from_hyper_request(req).await;

                match http_client.do_request(&req, *request_timeout).await? {
                    MyHttpResponse::Response(response) => {
                        return Ok(HttpResponse::Response(response));
                    }
                    MyHttpResponse::WebSocketUpgrade {
                        stream,
                        response,
                        disconnection,
                    } => {
                        http_client.upgraded_to_websocket();
                        return Ok(HttpResponse::WebSocketUpgrade {
                            stream: WebSocketUpgradeStream::SshChannel(stream),
                            response,
                            disconnection,
                        });
                    }
                }
            }
            HttpProxyPassContentSource::Http2OverSsh {
                app,
                over_ssh,
                ssh_session,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let http_client = app
                    .http2_over_ssh_clients_pool
                    .get(over_ssh.to_string().into(), *connect_timeout, || {
                        (
                            HttpOverSshConnector {
                                remote_endpoint: over_ssh.get_remote_endpoint().to_owned(),
                                debug: *debug,
                                ssh_session: ssh_session.clone(),
                                connect_timeout: *connect_timeout,
                            },
                            app.prometheus.clone(),
                        )
                    })
                    .await;

                let response = http_client.do_request(req, *request_timeout).await?;
                return Ok(HttpResponse::Response(response));
            }
            HttpProxyPassContentSource::LocalPath(src) => {
                let request_executor = src.get_request_executor(&req.uri())?;
                let result = request_executor.execute_request().await?;
                Ok(HttpResponse::Response(result.into()))
            }
            HttpProxyPassContentSource::PathOverSsh(src) => {
                let request_executor = src.get_request_executor(&req.uri()).await?;
                let result = request_executor.execute_request().await?;
                Ok(HttpResponse::Response(result.into()))
            }

            HttpProxyPassContentSource::PathOverGateway {
                gateway_id,
                path,
                default_file,
            } => {
                let result = super::executors::get_file_from_gateway(
                    app,
                    gateway_id,
                    path.as_str(),
                    default_file,
                    &req,
                )
                .await?;

                Ok(HttpResponse::Response(result.into()))
            }
            HttpProxyPassContentSource::Static(src) => {
                let request_executor = src.get_request_executor()?;
                let result = request_executor.execute_request().await?;
                Ok(HttpResponse::Response(result.into()))
            }
        }
    }
}

pub enum HttpResponse {
    Response(hyper::Response<BoxBody<Bytes, String>>),
    WebSocketUpgrade {
        stream: WebSocketUpgradeStream,
        response: hyper::Response<BoxBody<Bytes, String>>,
        disconnection: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    },
}

pub enum WebSocketUpgradeStream {
    TcpStream(tokio::net::TcpStream),
    TlsStream(my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>),
    SshChannel(SshAsyncChannel),
}
