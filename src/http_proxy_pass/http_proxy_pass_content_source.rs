use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use my_ssh::{ssh_settings::OverSshConnectionSettings, SshAsyncChannel, SshSession};
use rust_extensions::{remote_endpoint::RemoteEndpointOwned, StrOrString};

use crate::{
    consts::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT},
    http_client_connectors::{
        HttpConnector, HttpOverGatewayConnector, HttpOverSshConnector, HttpTlsConnector,
    },
    http_content_source::{LocalPathContentSrc, PathOverSshContentSource, StaticContentSrc},
    tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
};

use super::ProxyPassError;
use my_http_client::{http1::*, MyHttpClientDisconnect};

pub enum HttpProxyPassContentSource {
    Http1 {
        remote_endpoint: Arc<RemoteEndpointOwned>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Https1 {
        remote_endpoint: Arc<RemoteEndpointOwned>,
        domain_name: Option<String>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Http2 {
        remote_endpoint: Arc<RemoteEndpointOwned>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Https2 {
        remote_endpoint: Arc<RemoteEndpointOwned>,
        domain_name: Option<String>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Http1OverGateway {
        gateway_id: Arc<String>,
        remote_endpoint: Arc<RemoteEndpointOwned>,
    },

    Http2OverGateway {
        gateway_id: Arc<String>,
        remote_endpoint: Arc<RemoteEndpointOwned>,
    },

    Http1OverSsh {
        over_ssh: OverSshConnectionSettings,
        ssh_session: Arc<SshSession>,
        debug: bool,
        request_timeout: std::time::Duration,
        connect_timeout: std::time::Duration,
    },
    Http2OverSsh {
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
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        match self {
            HttpProxyPassContentSource::Http1 {
                remote_endpoint,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let mut http_client = crate::app::APP_CTX
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
                remote_endpoint,
                domain_name,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let mut http_client = crate::app::APP_CTX
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
                remote_endpoint,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let http_client = crate::app::APP_CTX
                    .http2_clients_pool
                    .get(remote_endpoint.as_str().into(), *connect_timeout, || {
                        (
                            HttpConnector {
                                remote_endpoint: remote_endpoint.clone(),
                                debug: *debug,
                            },
                            crate::app::APP_CTX.prometheus.clone(),
                        )
                    })
                    .await;

                let response = http_client.do_request(req, *request_timeout).await?;
                return Ok(HttpResponse::Response(response));
            }
            HttpProxyPassContentSource::Https2 {
                remote_endpoint,
                domain_name,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let http_client = crate::app::APP_CTX
                    .https2_clients_pool
                    .get(remote_endpoint.as_str().into(), *connect_timeout, || {
                        (
                            HttpTlsConnector {
                                remote_endpoint: remote_endpoint.clone(),
                                debug: *debug,
                                domain_name: domain_name.clone(),
                            },
                            crate::app::APP_CTX.prometheus.clone(),
                        )
                    })
                    .await;

                let response = http_client.do_request(req, *request_timeout).await?;
                return Ok(HttpResponse::Response(response));
            }
            HttpProxyPassContentSource::Http1OverSsh {
                over_ssh,
                ssh_session,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let mut http_client = crate::app::APP_CTX
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
                over_ssh,
                ssh_session,
                debug,
                request_timeout,
                connect_timeout,
            } => {
                let http_client = crate::app::APP_CTX
                    .http2_over_ssh_clients_pool
                    .get(over_ssh.to_string().into(), *connect_timeout, || {
                        (
                            HttpOverSshConnector {
                                remote_endpoint: over_ssh.get_remote_endpoint().to_owned(),
                                debug: *debug,
                                ssh_session: ssh_session.clone(),
                                connect_timeout: *connect_timeout,
                            },
                            crate::app::APP_CTX.prometheus.clone(),
                        )
                    })
                    .await;

                let response = http_client.do_request(req, *request_timeout).await?;
                return Ok(HttpResponse::Response(response));
            }
            Self::LocalPath(src) => {
                let request_executor = src.get_request_executor(&req.uri())?;
                let result = request_executor.execute_request().await?;
                Ok(HttpResponse::Response(result.into()))
            }
            Self::PathOverSsh(src) => {
                let request_executor = src.get_request_executor(&req.uri()).await?;
                let result = request_executor.execute_request().await?;
                Ok(HttpResponse::Response(result.into()))
            }
            Self::Http1OverGateway {
                gateway_id,
                remote_endpoint,
            } => {
                let id: StrOrString = format!(
                    "gateway:{}->{}",
                    gateway_id.as_str(),
                    remote_endpoint.as_str()
                )
                .into();
                let mut http_client = crate::app::APP_CTX
                    .http_over_gateway_clients_pool
                    .get(id, DEFAULT_HTTP_CONNECT_TIMEOUT, || {
                        HttpOverGatewayConnector {
                            remote_endpoint: remote_endpoint.clone(),
                            gateway_id: gateway_id.clone(),
                        }
                    })
                    .await;

                let req = MyHttpRequest::from_hyper_request(req).await;

                match http_client
                    .do_request(&req, DEFAULT_HTTP_REQUEST_TIMEOUT)
                    .await?
                {
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
                            stream: WebSocketUpgradeStream::HttpOverGatewayStream(stream),
                            response,
                            disconnection,
                        });
                    }
                }
            }

            Self::Http2OverGateway {
                gateway_id,
                remote_endpoint,
            } => {
                let http_client = crate::app::APP_CTX
                    .http2_over_gateway_clients_pool
                    .get(
                        remote_endpoint.as_str().into(),
                        DEFAULT_HTTP_CONNECT_TIMEOUT,
                        || {
                            (
                                HttpOverGatewayConnector {
                                    gateway_id: gateway_id.clone(),
                                    remote_endpoint: remote_endpoint.clone(),
                                },
                                crate::app::APP_CTX.prometheus.clone(),
                            )
                        },
                    )
                    .await;

                let response = http_client
                    .do_request(req, DEFAULT_HTTP_REQUEST_TIMEOUT)
                    .await?;
                return Ok(HttpResponse::Response(response));
            }

            Self::PathOverGateway {
                gateway_id,
                path,
                default_file,
            } => {
                let result = super::executors::get_file_from_gateway(
                    gateway_id,
                    path.as_str(),
                    default_file,
                    &req,
                )
                .await?;

                Ok(HttpResponse::Response(result.into()))
            }
            Self::Static(src) => {
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
    HttpOverGatewayStream(TcpGatewayProxyForwardStream),
}
