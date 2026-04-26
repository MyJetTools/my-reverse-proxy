use my_http_client::http1::*;

use crate::{http_proxy_pass::ProxyPassError, upstream_h1_pool::PoolKey};

use super::*;

pub struct Http1ContentSource {
    pub pool_key: PoolKey,
    pub request_timeout: std::time::Duration,
}

impl Http1ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let pool = crate::app::APP_CTX
            .h1_tcp_pools
            .get(&self.pool_key)
            .ok_or(ProxyPassError::UpstreamUnavailable)?;

        let is_ws = is_h1_websocket_upgrade(&req);

        let mut http_client = if is_ws {
            pool.create_connection()
                .await
                .map_err(|_| ProxyPassError::UpstreamUnavailable)?
        } else {
            match pool.get_connection() {
                Some(c) => c,
                None => pool
                    .create_connection()
                    .await
                    .map_err(|_| ProxyPassError::UpstreamUnavailable)?,
            }
        };

        let req = MyHttpRequest::from_hyper_request(req).await;

        match http_client.do_request(&req, self.request_timeout).await? {
            MyHttpResponse::Response(response) => Ok(HttpResponse::Response(response)),
            MyHttpResponse::WebSocketUpgrade {
                stream,
                response,
                disconnection,
            } => {
                http_client.upgraded_to_websocket();
                Ok(HttpResponse::WebSocketUpgrade {
                    stream: WebSocketUpgradeStream::TcpStream(stream),
                    response,
                    disconnection,
                })
            }
        }
    }
}
