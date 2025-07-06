use std::sync::Arc;

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::http_proxy_pass::ProxyPassError;

use super::*;

pub struct PathOverGatewayContentSource {
    pub gateway_id: Arc<String>,
    pub path: Arc<RemoteEndpointOwned>,
    pub default_file: Option<String>,
}

impl PathOverGatewayContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let result = crate::http_proxy_pass::executors::get_file_from_gateway(
            self.gateway_id.as_str(),
            self.path.as_str(),
            &self.default_file,
            &req,
        )
        .await?;

        Ok(HttpResponse::Response(result.into()))
    }
}
