use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use my_ssh::SshAsyncChannel;

use crate::{
    http_client::{Http1Client, Http2Client, SshConnector},
    http_content_source::{LocalPathContentSrc, PathOverSshContentSource, StaticContentSrc},
};

use super::ProxyPassError;
use my_http_client::{
    http1::{MyHttpClient, MyHttpResponse},
    http2::MyHttp2Client,
    MyHttpClientDisconnect,
};

pub enum HttpProxyPassContentSource {
    Http1(Http1Client),
    Http2(Http2Client),
    Http1OverSsh(MyHttpClient<SshAsyncChannel, SshConnector>),
    Http2OverSsh(MyHttp2Client<SshAsyncChannel, SshConnector>),
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
            HttpProxyPassContentSource::Http1(client) => match client {
                Http1Client::Http(my_http_client) => {
                    match my_http_client.do_request(req, request_timeout).await? {
                        MyHttpResponse::Response(response) => {
                            return Ok(HttpResponse::Response(response));
                        }
                        MyHttpResponse::WebSocketUpgrade {
                            stream,
                            response,
                            disconnection,
                        } => {
                            return Ok(HttpResponse::WebSocketUpgrade {
                                stream: WebSocketUpgradeStream::TcpStream(stream),
                                response,
                                disconnection,
                            })
                        }
                    }
                }
                Http1Client::Https(my_http_client) => {
                    match my_http_client.do_request(req, request_timeout).await? {
                        MyHttpResponse::Response(response) => {
                            return Ok(HttpResponse::Response(response));
                        }
                        MyHttpResponse::WebSocketUpgrade {
                            stream,
                            response,
                            disconnection,
                        } => {
                            return Ok(HttpResponse::WebSocketUpgrade {
                                stream: WebSocketUpgradeStream::TlsStream(stream),
                                response: response,
                                disconnection,
                            })
                        }
                    }
                }
            },
            HttpProxyPassContentSource::Http2(client) => match client {
                Http2Client::Http(my_http_client) => {
                    let response = my_http_client.do_request(req, request_timeout).await?;
                    return Ok(HttpResponse::Response(response));
                }
                Http2Client::Https(my_http_client) => {
                    let response = my_http_client.do_request(req, request_timeout).await?;
                    return Ok(HttpResponse::Response(response));
                }
            },
            HttpProxyPassContentSource::Http1OverSsh(client) => {
                match client.do_request(req, request_timeout).await? {
                    MyHttpResponse::Response(response) => {
                        return Ok(HttpResponse::Response(response));
                    }
                    MyHttpResponse::WebSocketUpgrade {
                        stream,
                        response,
                        disconnection,
                    } => {
                        return Ok(HttpResponse::WebSocketUpgrade {
                            stream: WebSocketUpgradeStream::SshChannel(stream),
                            response: response,
                            disconnection,
                        })
                    }
                }
            }
            HttpProxyPassContentSource::Http2OverSsh(client) => {
                let result = client.do_request(req, request_timeout).await?;
                return Ok(HttpResponse::Response(result.into()));
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
