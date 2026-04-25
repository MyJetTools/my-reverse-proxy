use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use my_http_client::MyHttpClientDisconnect;
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{http_client_connectors::HttpConnector, http_proxy_pass::ProxyPassError};

use super::*;

pub struct Http2ContentSource {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
    pub debug: bool,
    pub request_timeout: std::time::Duration,
    pub connect_timeout: std::time::Duration,
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

impl Http2ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let http_client = crate::app::APP_CTX
            .http2_clients_pool
            .get(
                self.remote_endpoint.as_str().into(),
                self.connect_timeout,
                || {
                    (
                        HttpConnector {
                            remote_endpoint: self.remote_endpoint.clone(),
                            debug: self.debug,
                        },
                        crate::app::APP_CTX.prometheus.clone(),
                    )
                },
            );

        if is_h2_extended_connect(&req) {
            let path = req
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str().to_string())
                .unwrap_or_else(|| "/".to_string());

            let headers = req.headers().clone();

            let upgraded = http_client
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

        let response = http_client.do_request(req, self.request_timeout).await?;
        return Ok(HttpResponse::Response(response));
    }
}
