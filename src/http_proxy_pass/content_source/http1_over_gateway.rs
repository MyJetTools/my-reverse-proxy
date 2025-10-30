use std::sync::Arc;

use my_http_client::http1::*;
use rust_extensions::{remote_endpoint::RemoteEndpointOwned, StrOrString};

use crate::{
    consts::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT},
    http_proxy_pass::ProxyPassError,
};

use super::*;

pub struct Http1OverGatewayContentSource {
    pub gateway_id: Arc<String>,
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
}

impl Http1OverGatewayContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let id: StrOrString = format!(
            "gateway:{}->{}",
            self.gateway_id.as_str(),
            self.remote_endpoint.as_str()
        )
        .into();
        let mut http_client = crate::app::APP_CTX
            .http_over_gateway_clients_pool
            .get(id, DEFAULT_HTTP_CONNECT_TIMEOUT, || {
                HttpOverGatewayConnector {
                    remote_endpoint: self.remote_endpoint.clone(),
                    gateway_id: self.gateway_id.clone(),
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
}
