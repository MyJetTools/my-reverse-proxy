use std::sync::Arc;

use my_http_client::http1::*;
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{http_client_connectors::HttpTlsConnector, http_proxy_pass::ProxyPassError};

use super::*;

pub struct Https1ContentSource {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
    pub debug: bool,
    pub request_timeout: std::time::Duration,
    pub connect_timeout: std::time::Duration,
    pub domain_name: Option<String>,
}

impl Https1ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let mut http_client = crate::app::APP_CTX
            .https_clients_pool
            .get(
                self.remote_endpoint.as_str().into(),
                self.connect_timeout,
                || HttpTlsConnector {
                    remote_endpoint: self.remote_endpoint.clone(),
                    debug: self.debug,
                    domain_name: self.domain_name.clone(),
                },
            )
            .await;

        let req = MyHttpRequest::from_hyper_request(req).await;

        match http_client.do_request(&req, self.request_timeout).await? {
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
}
