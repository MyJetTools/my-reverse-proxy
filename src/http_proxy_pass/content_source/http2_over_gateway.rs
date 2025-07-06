use std::sync::Arc;

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{
    consts::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT},
    http_client_connectors::HttpOverGatewayConnector,
    http_proxy_pass::ProxyPassError,
};

use super::*;

pub struct Http2OverGatewayContentSource {
    pub gateway_id: Arc<String>,
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
}

impl Http2OverGatewayContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let http_client = crate::app::APP_CTX
            .http2_over_gateway_clients_pool
            .get(
                self.remote_endpoint.as_str().into(),
                DEFAULT_HTTP_CONNECT_TIMEOUT,
                || {
                    (
                        HttpOverGatewayConnector {
                            gateway_id: self.gateway_id.clone(),
                            remote_endpoint: self.remote_endpoint.clone(),
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
}
