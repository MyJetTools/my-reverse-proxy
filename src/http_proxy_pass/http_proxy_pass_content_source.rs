use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use my_ssh::{SshAsyncChannel, SshSession};
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{
    app::AppContext,
    http_client::{HttpConnector, HttpTlsConnector, SshConnector},
    http_content_source::{LocalPathContentSrc, PathOverSshContentSource, StaticContentSrc},
};

use super::ProxyPassError;
use my_http_client::{http1::*, MyHttpClientDisconnect};

pub enum HttpProxyPassContentSource {
    Http1 {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        debug: bool,
    },
    Https1 {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        domain_name: Option<String>,
        debug: bool,
    },
    Http2 {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        debug: bool,
    },
    Https2 {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        domain_name: Option<String>,
        debug: bool,
    },
    Http1OverSsh {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        ssh_session: Arc<SshSession>,
        debug: bool,
    },
    Http2OverSsh {
        app: Arc<AppContext>,
        remote_endpoint: RemoteEndpointOwned,
        ssh_session: Arc<SshSession>,
        debug: bool,
    },
    LocalPath(LocalPathContentSrc),
    PathOverSsh(PathOverSshContentSource),
    Static(StaticContentSrc),
}

impl HttpProxyPassContentSource {
    pub async fn send_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: std::time::Duration,
    ) -> Result<HttpResponse, ProxyPassError> {
        match self {
            HttpProxyPassContentSource::Http1 {
                app,
                remote_endpoint,
                debug,
            } => {
                let mut http_client = app
                    .http_clients_pool
                    .get(remote_endpoint.to_ref(), || HttpConnector {
                        remote_endpoint: remote_endpoint.clone(),
                        debug: *debug,
                    })
                    .await;

                let req = MyHttpRequest::from_hyper_request(req).await;

                match http_client.do_request(&req, request_timeout).await? {
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
            } => {
                let mut http_client = app
                    .https_clients_pool
                    .get(remote_endpoint.to_ref(), || HttpTlsConnector {
                        remote_endpoint: remote_endpoint.clone(),
                        debug: *debug,
                        domain_name: domain_name.clone(),
                    })
                    .await;

                let req = MyHttpRequest::from_hyper_request(req).await;

                match http_client.do_request(&req, request_timeout).await? {
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
            } => {
                let http_client = app
                    .http2_clients_pool
                    .get(remote_endpoint.to_ref(), || {
                        (
                            HttpConnector {
                                remote_endpoint: remote_endpoint.clone(),
                                debug: *debug,
                            },
                            app.prometheus.clone(),
                        )
                    })
                    .await;

                let response = http_client.do_request(req, request_timeout).await?;
                return Ok(HttpResponse::Response(response));
            }
            HttpProxyPassContentSource::Https2 {
                app,
                remote_endpoint,
                domain_name,
                debug,
            } => {
                let http_client = app
                    .https2_clients_pool
                    .get(remote_endpoint.to_ref(), || {
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

                let response = http_client.do_request(req, request_timeout).await?;
                return Ok(HttpResponse::Response(response));
            }
            HttpProxyPassContentSource::Http1OverSsh {
                app,
                remote_endpoint,
                ssh_session,
                debug,
            } => {
                let mut http_client = app
                    .http_over_ssh_clients_pool
                    .get(remote_endpoint.to_ref(), || SshConnector {
                        remote_endpoint: remote_endpoint.clone(),
                        debug: *debug,
                        ssh_session: ssh_session.clone(),
                    })
                    .await;

                let req = MyHttpRequest::from_hyper_request(req).await;

                match http_client.do_request(&req, request_timeout).await? {
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
                remote_endpoint,
                ssh_session,
                debug,
            } => {
                let http_client = app
                    .http2_over_ssh_clients_pool
                    .get(remote_endpoint.to_ref(), || {
                        (
                            SshConnector {
                                remote_endpoint: remote_endpoint.clone(),
                                debug: *debug,
                                ssh_session: ssh_session.clone(),
                            },
                            app.prometheus.clone(),
                        )
                    })
                    .await;

                let response = http_client.do_request(req, request_timeout).await?;
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
