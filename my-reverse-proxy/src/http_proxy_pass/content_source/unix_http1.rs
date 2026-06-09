use my_http_client::http1::*;

use crate::{
    http_client_connectors::UnixSocketHttpConnector,
    http_proxy_pass::ProxyPassError,
    upstream_h1_pool::{ConnectorFactory, PoolDesc, PoolParams},
};

use super::*;

pub struct UnixHttp1ContentSource {
    pub pool_desc: PoolDesc,
    pub pool_params: PoolParams,
    pub factory: ConnectorFactory<UnixSocketHttpConnector>,
    pub request_timeout: std::time::Duration,
    /// `true` when this source backs an `mcp` location — such requests get a
    /// dedicated non-pooled connection (see `execute`).
    pub is_mcp: bool,
}

impl UnixHttp1ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let pool = match crate::app::APP_CTX.h1_uds_pools.get(self.pool_desc.location_id) {
            Some(p) => p,
            None => crate::app::APP_CTX.h1_uds_pools.ensure_pool(
                self.pool_desc.clone(),
                self.pool_params.clone(),
                self.factory.clone(),
            ),
        };

        let is_ws = is_h1_websocket_upgrade(&req);

        let handle = if is_ws {
            pool.create_ws_connection().await
        } else if self.is_mcp {
            // MCP responses can be infinite SSE streams; give each request its
            // own connection so concurrent JSON-RPC POSTs never pipeline behind
            // an in-flight stream on a shared pooled connection.
            pool.create_dedicated_connection().await
        } else {
            pool.get_connection().await
        }
        .map_err(|_| ProxyPassError::UpstreamUnavailable)?;

        let req = MyHttpRequest::from_hyper_request(req).await;

        match handle.do_request(&req, self.request_timeout).await? {
            MyHttpResponse::Response(response) => {
                // Tie the pool handle to the body lifetime so the entry is not
                // released while a streaming body is still being read.
                let response = attach_conn_guard(response, Box::new(handle));
                Ok(HttpResponse::Response(response))
            }
            MyHttpResponse::WebSocketUpgrade {
                stream,
                response,
                disconnection,
            } => Ok(HttpResponse::WebSocketUpgrade {
                stream: WebSocketUpgradeStream::UnixStream(stream),
                response,
                disconnection,
            }),
        }
    }
}
