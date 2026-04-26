use std::sync::Arc;

use crate::{http_proxy_pass::ProxyPassError, upstream_h2_pool::PoolKey};

use super::*;

pub struct UnixHttp2ContentSource {
    pub pool_key: PoolKey,
    pub request_timeout: std::time::Duration,
}

impl UnixHttp2ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let pool = crate::app::APP_CTX
            .h2_uds_pools
            .get(&self.pool_key)
            .ok_or(ProxyPassError::UpstreamUnavailable)?;

        let is_ws = is_h2_extended_connect(&req);

        let client = if is_ws {
            pool.create_connection()
                .await
                .map_err(|_| ProxyPassError::UpstreamUnavailable)?
        } else {
            pool.get_connection()
                .ok_or(ProxyPassError::UpstreamUnavailable)?
        };

        let mut response = execute_h2(&client, req, self.request_timeout).await?;

        if is_ws {
            if let HttpResponse::WebSocketUpgrade { disconnection, .. } = &mut response {
                *disconnection = Arc::new(H2WsActiveGuard::new(
                    self.pool_key.endpoint_label(),
                    client,
                ));
            }
        }

        Ok(response)
    }
}
