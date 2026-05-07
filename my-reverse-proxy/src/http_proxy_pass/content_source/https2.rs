use std::sync::{atomic::Ordering, Arc};

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    http_client_connectors::HttpTlsConnector,
    http_proxy_pass::ProxyPassError,
    upstream_h2_pool::{ConnectorFactory, PoolDesc, PoolParams},
};

use super::*;

pub struct Https2ContentSource {
    pub pool_desc: PoolDesc,
    pub pool_params: PoolParams,
    pub factory: ConnectorFactory<HttpTlsConnector>,
    pub request_timeout: std::time::Duration,
}

impl Https2ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let pool = match crate::app::APP_CTX.h2_tls_pools.get(self.pool_desc.location_id) {
            Some(p) => p,
            None => crate::app::APP_CTX.h2_tls_pools.ensure_pool(
                self.pool_desc.clone(),
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
                    self.pool_desc.name.clone(),
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
