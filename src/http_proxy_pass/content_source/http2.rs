use std::sync::Arc;

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{http_client_connectors::HttpConnector, http_proxy_pass::ProxyPassError};

use super::*;

pub struct Http2ContentSource {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
    pub debug: bool,
    pub request_timeout: std::time::Duration,
    pub connect_timeout: std::time::Duration,
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
            )
            .await;

        let response = http_client.do_request(req, self.request_timeout).await?;
        return Ok(HttpResponse::Response(response));
    }
}
