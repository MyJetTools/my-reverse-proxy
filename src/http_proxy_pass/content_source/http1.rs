use my_http_client::http1::*;

use crate::{
    http_client_connectors::HttpConnector,
    http_proxy_pass::ProxyPassError,
    upstream_h1_pool::{ConnectorFactory, PoolDesc, PoolParams},
};

use super::*;

pub struct Http1ContentSource {
    pub pool_desc: PoolDesc,
    pub pool_params: PoolParams,
    pub factory: ConnectorFactory<HttpConnector>,
    pub request_timeout: std::time::Duration,
}

impl Http1ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let pool = match crate::app::APP_CTX.h1_tcp_pools.get(self.pool_desc.location_id) {
            Some(p) => p,
            None => crate::app::APP_CTX.h1_tcp_pools.ensure_pool(
                self.pool_desc.clone(),
                self.pool_params.clone(),
                self.factory.clone(),
            ),
        };

        let is_ws = is_h1_websocket_upgrade(&req);

        let handle = if is_ws {
            pool.create_ws_connection().await
        } else {
            pool.get_connection().await
        }
        .map_err(|_| ProxyPassError::UpstreamUnavailable)?;

        let req = MyHttpRequest::from_hyper_request(req).await;

        match handle.do_request(&req, self.request_timeout).await? {
            MyHttpResponse::Response(response) => Ok(HttpResponse::Response(response)),
            MyHttpResponse::WebSocketUpgrade {
                stream,
                response,
                disconnection,
            } => Ok(HttpResponse::WebSocketUpgrade {
                stream: WebSocketUpgradeStream::TcpStream(stream),
                response,
                disconnection,
            }),
        }
    }
}
