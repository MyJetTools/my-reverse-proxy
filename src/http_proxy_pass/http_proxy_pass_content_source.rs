use bytes::Bytes;
use http::Response;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::body::Incoming;
use my_ssh::SshAsyncChannel;

use tokio::sync::Mutex;

use crate::{
    http_client::{Http1Client, Http2Client, Ssh1Connector, HTTP_CLIENT_TIMEOUT},
    http_content_source::{LocalPathContentSrc, PathOverSshContentSource, StaticContentSrc},
    my_http_client::MyHttpClient,
};

use super::ProxyPassError;

pub enum HttpProxyPassContentSource {
    Http1(Http1Client),

    Http2(Mutex<Http2Client>),

    Http1OverSsh(MyHttpClient<SshAsyncChannel, Ssh1Connector>),
    LocalPath(LocalPathContentSrc),
    PathOverSsh(PathOverSshContentSource),
    Static(StaticContentSrc),
}

impl HttpProxyPassContentSource {
    pub async fn upgrade_websocket(
        &self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<
        (
            WebSocketUpgradeStream,
            hyper::Response<BoxBody<Bytes, String>>,
        ),
        ProxyPassError,
    > {
        match self {
            HttpProxyPassContentSource::Http1(client) => match client {
                Http1Client::Http(client) => {
                    let result = client
                        .upgrade_to_web_socket(req, |read, write| read.unsplit(write))
                        .await?;

                    Ok((WebSocketUpgradeStream::TcpStream(result.0), result.1))
                }
                Http1Client::Https(client) => {
                    let result = client
                        .upgrade_to_web_socket(req, |read, write| read.unsplit(write))
                        .await?;

                    Ok((WebSocketUpgradeStream::TlsStream(result.0), result.1))
                }
            },
            HttpProxyPassContentSource::Http1OverSsh(client) => {
                let result = client
                    .upgrade_to_web_socket(req, |read, write| read.unsplit(write))
                    .await?;

                Ok((WebSocketUpgradeStream::SshChannel(result.0), result.1))
            }
            _ => panic!("Not implemented"),
        }
    }

    pub async fn send_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<hyper::Response<BoxBody<Bytes, String>>, ProxyPassError> {
        match self {
            HttpProxyPassContentSource::Http1(client) => {
                let result = match client {
                    Http1Client::Http(my_http_client) => {
                        let feature = my_http_client.send(req);
                        tokio::time::timeout(HTTP_CLIENT_TIMEOUT, feature).await
                    }
                    Http1Client::Https(my_http_client) => {
                        let feature = my_http_client.send(req);
                        tokio::time::timeout(HTTP_CLIENT_TIMEOUT, feature).await
                    }
                };

                if result.is_err() {
                    return Err(ProxyPassError::Timeout);
                }
                let result = result.unwrap();

                return Ok(result?);
            }
            HttpProxyPassContentSource::Http2(client) => {
                let feature = {
                    let mut client = client.lock().await;
                    client.send_request.send_request(req)
                };
                let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, feature).await;

                if result.is_err() {
                    return Err(ProxyPassError::Timeout);
                }
                let result = result.unwrap();
                let response = result?;
                return Ok(from_incoming_body(response));
            }
            HttpProxyPassContentSource::Http1OverSsh(client) => {
                let feature = client.send(req);
                let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, feature).await;

                if result.is_err() {
                    return Err(ProxyPassError::Timeout);
                }
                let result = result.unwrap()?;

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

fn from_incoming_body(response: Response<Incoming>) -> Response<BoxBody<Bytes, String>> {
    let (parts, body) = response.into_parts();

    let box_body = body.map_err(|e| e.to_string()).boxed();

    Response::from_parts(parts, box_body)
}

pub enum WebSocketUpgradeStream {
    TcpStream(tokio::net::TcpStream),
    TlsStream(my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>),
    SshChannel(SshAsyncChannel),
}
