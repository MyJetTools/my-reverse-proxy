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
    /// `true` when this h1 source backs an `mcp` location — used only to gate
    /// the upstream tracing logs.
    pub is_mcp: bool,
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

        let method = req.method().clone();
        let uri = req.uri().clone();

        let handle = if is_ws {
            pool.create_ws_connection().await
        } else {
            pool.get_connection().await
        }
        .map_err(|_| {
            if self.is_mcp {
                println!(
                    "MCP upstream UNAVAILABLE (no connection): {} {} -> [{}]",
                    method, uri, self.pool_desc.id_string
                );
            }
            ProxyPassError::UpstreamUnavailable
        })?;

        if self.is_mcp {
            println!(
                "MCP upstream reached, sending: {} {} -> [{}]",
                method, uri, self.pool_desc.id_string
            );
        }

        let req = MyHttpRequest::from_hyper_request(req).await;

        let response = match handle.do_request(&req, self.request_timeout).await {
            Ok(response) => response,
            Err(err) => {
                if self.is_mcp {
                    println!(
                        "MCP upstream request FAILED: {} {} -> [{}]: {:?}",
                        method, uri, self.pool_desc.id_string, err
                    );
                }
                return Err(err.into());
            }
        };

        match response {
            MyHttpResponse::Response(response) => {
                if self.is_mcp {
                    println!(
                        "MCP upstream OK: {} {} -> [{}] status {}",
                        method,
                        uri,
                        self.pool_desc.id_string,
                        response.status()
                    );
                }
                Ok(HttpResponse::Response(response))
            }
            MyHttpResponse::WebSocketUpgrade {
                stream,
                response,
                disconnection,
            } => {
                if self.is_mcp {
                    println!(
                        "MCP upstream WS-UPGRADE: {} {} -> [{}]",
                        method, uri, self.pool_desc.id_string
                    );
                }
                Ok(HttpResponse::WebSocketUpgrade {
                    stream: WebSocketUpgradeStream::TcpStream(stream),
                    response,
                    disconnection,
                })
            }
        }
    }
}
