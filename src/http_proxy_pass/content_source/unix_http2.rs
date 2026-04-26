use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use my_http_client::MyHttpClientDisconnect;
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{app::APP_CTX, http_client_connectors::*, http_proxy_pass::ProxyPassError};

use super::*;

pub struct UnixHttp2ContentSource {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
    pub debug: bool,
    pub request_timeout: std::time::Duration,
    pub connect_timeout: std::time::Duration,
    pub connection_id: i64,
}

struct H2NoopDisconnect;

impl MyHttpClientDisconnect for H2NoopDisconnect {
    fn disconnect(&self) {}
    fn web_socket_disconnect(&self) {}
    fn get_connection_id(&self) -> u64 {
        0
    }
}

fn is_h2_extended_connect(req: &http::Request<http_body_util::Full<bytes::Bytes>>) -> bool {
    if req.method() != hyper::Method::CONNECT {
        return false;
    }
    match req.extensions().get::<hyper::ext::Protocol>() {
        Some(p) => p.as_ref().eq_ignore_ascii_case(b"websocket"),
        None => false,
    }
}

impl UnixHttp2ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let h2_client = crate::app::APP_CTX
            .unix_socket_h2_socket_per_connection
            .get_or_create(self.connection_id, || {
                let mut result: my_http_client::http2::MyHttp2Client<
                    tokio::net::UnixStream,
                    UnixSocketHttpConnector,
                > = UnixSocketHttpConnector {
                    remote_endpoint: self.remote_endpoint.to_owned(),
                    debug: self.debug,
                }
                .into();

                result.set_connect_timeout(self.connect_timeout);

                result
            });

        if is_h2_extended_connect(&req) {
            let path = req
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str().to_string())
                .unwrap_or_else(|| "/".to_string());

            let headers = req.headers().clone();

            let upgraded = h2_client
                .do_extended_connect(&path, headers, self.request_timeout)
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

        let http_response = h2_client.do_request(req, self.request_timeout).await?;

        Ok(HttpResponse::Response(http_response))
    }
}

impl Drop for UnixHttp2ContentSource {
    fn drop(&mut self) {
        APP_CTX
            .unix_socket_h2_socket_per_connection
            .remove(self.connection_id);
    }
}
