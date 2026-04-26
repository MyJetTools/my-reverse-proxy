use crate::{http_proxy_pass::ProxyPassError, upstream_h2_pool::PoolKey};

use super::*;

pub struct Https2ContentSource {
    pub pool_key: PoolKey,
    pub request_timeout: std::time::Duration,
}

impl Https2ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let pool = crate::app::APP_CTX
            .h2_tls_pools
            .get(&self.pool_key)
            .ok_or(ProxyPassError::UpstreamUnavailable)?;

        let client = pool.acquire().ok_or(ProxyPassError::UpstreamUnavailable)?;

        execute_h2(&client, req, self.request_timeout).await
    }
}
