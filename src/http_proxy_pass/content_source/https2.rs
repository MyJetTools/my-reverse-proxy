use std::sync::Arc;

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{http_client_connectors::HttpTlsConnector, http_proxy_pass::ProxyPassError};

use super::*;

pub struct Https2ContentSource {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
    pub debug: bool,
    pub request_timeout: std::time::Duration,
    pub connect_timeout: std::time::Duration,
    pub domain_name: Option<String>,
}

impl Https2ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let http_client = crate::app::APP_CTX
            .https2_clients_pool
            .get(
                self.remote_endpoint.as_str().into(),
                self.connect_timeout,
                || {
                    (
                        HttpTlsConnector {
                            remote_endpoint: self.remote_endpoint.clone(),
                            debug: self.debug,
                            domain_name: self.domain_name.clone(),
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
