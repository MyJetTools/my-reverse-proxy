use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full};
use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector, MyHttpClientDisconnect};
use my_http_client::{HyperResponse, MyHttpClientError};

use crate::http_proxy_pass::ProxyPassError;
use crate::http2_client_pool::Http2ClientPoolItem;

use super::{HttpResponse, WebSocketUpgradeStream};

#[async_trait::async_trait]
pub trait H2Sender: Send + Sync {
    async fn do_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperResponse, MyHttpClientError>;

    async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError>;
}

#[async_trait::async_trait]
impl<TStream, TConnector> H2Sender for Http2ClientPoolItem<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    async fn do_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperResponse, MyHttpClientError> {
        Http2ClientPoolItem::do_request(self, req, request_timeout).await
    }

    async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        Http2ClientPoolItem::do_extended_connect(self, path, headers, request_timeout).await
    }
}

#[async_trait::async_trait]
impl<TStream, TConnector> H2Sender for Arc<MyHttp2Client<TStream, TConnector>>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    async fn do_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperResponse, MyHttpClientError> {
        MyHttp2Client::do_request(self.as_ref(), req, request_timeout).await
    }

    async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        MyHttp2Client::do_extended_connect(self.as_ref(), path, headers, request_timeout).await
    }
}

struct H2NoopDisconnect;

impl MyHttpClientDisconnect for H2NoopDisconnect {
    fn disconnect(&self) {}
    fn web_socket_disconnect(&self) {}
    fn get_connection_id(&self) -> u64 {
        0
    }
}

fn is_h2_extended_connect(req: &hyper::Request<Full<Bytes>>) -> bool {
    if req.method() != hyper::Method::CONNECT {
        return false;
    }
    match req.extensions().get::<hyper::ext::Protocol>() {
        Some(p) => p.as_ref().eq_ignore_ascii_case(b"websocket"),
        None => false,
    }
}

pub async fn execute_h2(
    sender: &impl H2Sender,
    req: hyper::Request<Full<Bytes>>,
    request_timeout: Duration,
) -> Result<HttpResponse, ProxyPassError> {
    if is_h2_extended_connect(&req) {
        let path = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());

        let headers = req.headers().clone();

        let upgraded = sender
            .do_extended_connect(&path, headers, request_timeout)
            .await?;

        let response = hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .body(Empty::<Bytes>::new().map_err(|never| match never {}).boxed())
            .unwrap();

        let disconnection: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static> =
            Arc::new(H2NoopDisconnect);

        return Ok(HttpResponse::WebSocketUpgrade {
            stream: WebSocketUpgradeStream::H2Upgraded(upgraded),
            response,
            disconnection,
        });
    }

    let response = sender.do_request(req, request_timeout).await?;
    Ok(HttpResponse::Response(response))
}
