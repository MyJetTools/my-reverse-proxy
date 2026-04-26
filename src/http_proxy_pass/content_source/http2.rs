use std::sync::{atomic::Ordering, Arc};

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    http_client_connectors::HttpConnector,
    http_proxy_pass::ProxyPassError,
    upstream_h2_pool::{ConnectorFactory, PoolKey, PoolParams},
};

use super::*;

pub struct Http2ContentSource {
    pub pool_key: PoolKey,
    pub pool_params: PoolParams,
    pub factory: ConnectorFactory<HttpConnector>,
    pub request_timeout: std::time::Duration,
}

impl Http2ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let pool = match crate::app::APP_CTX.h2_tcp_pools.get(&self.pool_key) {
            Some(p) => p,
            None => crate::app::APP_CTX.h2_tcp_pools.ensure_pool(
                self.pool_key.clone(),
                self.pool_params.clone(),
                self.factory.clone(),
            ),
        };

        let is_ws = is_h2_extended_connect(&req);

        if is_ws {
            let client = pool
                .create_connection()
                .await
                .map_err(|_| ProxyPassError::UpstreamUnavailable)?;
            let mut response = execute_h2(&client, req, self.request_timeout).await?;
            if let HttpResponse::WebSocketUpgrade { disconnection, .. } = &mut response {
                *disconnection = Arc::new(H2WsActiveGuard::new(
                    self.pool_key.endpoint_label(),
                    client,
                ));
            }
            return Ok(response);
        }

        let entry = pool
            .get_connection()
            .await
            .map_err(|_| ProxyPassError::UpstreamUnavailable)?;
        let client = entry.client.load_full();
        let result = execute_h2(&client, req, self.request_timeout).await;
        match &result {
            Ok(_) => entry.last_success.update(DateTimeAsMicroseconds::now()),
            Err(_) => entry.dead.store(true, Ordering::Relaxed),
        }
        result
    }
}
