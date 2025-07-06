use std::sync::Arc;

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{app::APP_CTX, http_client_connectors::*, http_proxy_pass::ProxyPassError};

use super::*;

pub struct UnixHttp2ContentSource {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,
    pub debug: bool,
    pub request_timeout: std::time::Duration,
    pub connect_timeout: std::time::Duration,
    pub connection_id: i64,
}

impl UnixHttp2ContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let h2_client = crate::app::APP_CTX
            .unix_socket_h2_socket_per_connection
            .get_or_create(self.connection_id, || {
                let mut result: my_http_client::http2::MyHttp2Client<
                    tokio::net::UnixStream,
                    UnixSocketHttpConnector,
                > = UnixSocketHttpConnector {
                    remote_endpoint: self.remote_endpoint.to_owned(),
                    debug: self.debug,
                }
                .into();

                result.set_connect_timeout(self.connect_timeout);

                result
            })
            .await;

        let http_response = h2_client.do_request(req, self.request_timeout).await?;

        Ok(HttpResponse::Response(http_response))
    }
}

impl Drop for UnixHttp2ContentSource {
    fn drop(&mut self) {
        let connection_id = self.connection_id;

        tokio::spawn(async move {
            APP_CTX
                .unix_socket_h2_socket_per_connection
                .remove(connection_id)
                .await;
        });
    }
}
