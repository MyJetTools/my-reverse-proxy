use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use my_ssh::SshAsyncChannel;

use crate::{
    http_client::{Http1Client, Http2Client, SshConnector},
    http_content_source::{LocalPathContentSrc, PathOverSshContentSource, StaticContentSrc},
};

use super::ProxyPassError;
use my_http_client::{http1::MyHttpClient, http2::MyHttp2Client, MyHttpClientDisconnect};

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
    pub async fn upgrade_websocket(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: std::time::Duration,
    ) -> Result<
        (
            WebSocketUpgradeStream,
            hyper::Response<BoxBody<Bytes, String>>,
            Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
        ),
        ProxyPassError,
    > {
        match self {
            HttpProxyPassContentSource::Http1(client) => match client {
                Http1Client::Http(client) => {
                    let result = client
                        .upgrade_to_web_socket(req, request_timeout, |read, write| {
                            read.unsplit(write)
                        })
                        .await?;

                    Ok((
                        WebSocketUpgradeStream::TcpStream(result.0),
                        result.1,
                        Arc::new(result.2),
                    ))
                }
                Http1Client::Https(client) => {
                    let result = client
                        .upgrade_to_web_socket(req, request_timeout, |read, write| {
                            read.unsplit(write)
                        })
                        .await?;

                    Ok((
                        WebSocketUpgradeStream::TlsStream(result.0),
                        result.1,
                        Arc::new(result.2),
                    ))
                }
            },
            HttpProxyPassContentSource::Http1OverSsh(client) => {
                let result = client
                    .upgrade_to_web_socket(req, request_timeout, |read, write| read.unsplit(write))
                    .await?;

                Ok((
                    WebSocketUpgradeStream::SshChannel(result.0),
                    result.1,
                    Arc::new(result.2),
                ))
            }
            _ => panic!("Not implemented"),
        }
    }

    pub async fn send_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: std::time::Duration,
    ) -> Result<hyper::Response<BoxBody<Bytes, String>>, ProxyPassError> {
        match self {
            HttpProxyPassContentSource::Http1(client) => match client {
                Http1Client::Http(my_http_client) => {
                    let result = my_http_client.send(req, request_timeout).await?;
                    return Ok(result);
                }
                Http1Client::Https(my_http_client) => {
                    let result = my_http_client.send(req, request_timeout).await?;
                    return Ok(result);
                }
            },
            HttpProxyPassContentSource::Http2(client) => match client {
                Http2Client::Http(my_http_client) => {
                    let result = my_http_client.send(req, request_timeout).await?;
                    return Ok(result);
                }
                Http2Client::Https(my_http_client) => {
                    let result = my_http_client.send(req, request_timeout).await?;
                    return Ok(result);
                }
            },
            HttpProxyPassContentSource::Http1OverSsh(client) => {
                let result = client.send(req, request_timeout).await?;
                return Ok(result);
            }
            HttpProxyPassContentSource::Http2OverSsh(client) => {
                let result = client.send(req, request_timeout).await?;
                return Ok(result);
            }
            HttpProxyPassContentSource::LocalPath(src) => {
                let request_executor = src.get_request_executor(&req.uri())?;
                let result = request_executor.execute_request().await?;
                Ok(result.into())
            }
            HttpProxyPassContentSource::PathOverSsh(src) => {
                let request_executor = src.get_request_executor(&req.uri())?;
                let result = request_executor.execute_request().await?;
                Ok(result.into())
            }
            HttpProxyPassContentSource::Static(src) => {
                let request_executor = src.get_request_executor()?;
                let result = request_executor.execute_request().await?;
                Ok(result.into())
            }
        }
    }
}

pub enum WebSocketUpgradeStream {
    TcpStream(tokio::net::TcpStream),
    TlsStream(my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>),
    SshChannel(SshAsyncChannel),
}
