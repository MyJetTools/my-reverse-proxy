use std::sync::Arc;

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{
    app::APP_CTX, http_client_connectors::UnixSocketHttpConnector, http_proxy_pass::ProxyPassError,
};

use super::*;

pub struct UnixHttp1ContentSource {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
    pub debug: bool,
    pub request_timeout: std::time::Duration,
    pub connect_timeout: std::time::Duration,
    pub connection_id: i64,
}

impl UnixHttp1ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let http_client = crate::app::APP_CTX
            .unix_sockets_per_connection
            .get_or_create(self.connection_id, || {
                let mut http_client: my_http_client::http1_hyper::MyHttpHyperClient<
                    tokio::net::UnixStream,
                    UnixSocketHttpConnector,
                > = UnixSocketHttpConnector {
                    remote_endpoint: self.remote_endpoint.clone(),
                    debug: self.debug,
                }
                .into();

                http_client.set_connect_timeout(self.connect_timeout);

                http_client
            })
            .await;

        let response = http_client
            .do_request(req.clone(), self.request_timeout)
            .await?;

        match response {
            my_http_client::http1_hyper::HyperHttpResponse::Response(response) => {
                return Ok(HttpResponse::Response(response));
            }
            my_http_client::http1_hyper::HyperHttpResponse::WebSocketUpgrade {
                response,
                web_socket,
            } => {
                return Ok(HttpResponse::WebSocketUpgrade {
                    stream: WebSocketUpgradeStream::Hyper(web_socket),
                    response,
                    disconnection: http_client,
                });
            }
        }
    }
}

impl Drop for UnixHttp1ContentSource {
    fn drop(&mut self) {
        let connection_id = self.connection_id;

        tokio::spawn(async move {
            APP_CTX
                .unix_sockets_per_connection
                .remove(connection_id)
                .await;
        });
    }
}
