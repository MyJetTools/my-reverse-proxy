use std::sync::Arc;

use my_http_client::http1::*;
use my_ssh::{ssh_settings::OverSshConnectionSettings, SshSession};

use crate::{http_client_connectors::HttpOverSshConnector, http_proxy_pass::ProxyPassError};

use super::*;

pub struct Http1OverSshContentSource {
    pub over_ssh: OverSshConnectionSettings,
    pub ssh_session: Arc<SshSession>,
    pub debug: bool,
    pub request_timeout: std::time::Duration,
    pub connect_timeout: std::time::Duration,
}

impl Http1OverSshContentSource {
    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let mut http_client = crate::app::APP_CTX
            .http_over_ssh_clients_pool
            .get(
                self.over_ssh.to_string().into(),
                self.connect_timeout,
                || HttpOverSshConnector {
                    remote_endpoint: self.over_ssh.get_remote_endpoint().to_owned(),
                    debug: self.debug,
                    ssh_session: self.ssh_session.clone(),
                    connect_timeout: self.connect_timeout,
                },
            )
            .await;

        let req = MyHttpRequest::from_hyper_request(req).await;

        match http_client.do_request(&req, self.request_timeout).await? {
            MyHttpResponse::Response(response) => {
                return Ok(HttpResponse::Response(response));
            }
            MyHttpResponse::WebSocketUpgrade {
                stream,
                response,
                disconnection,
            } => {
                http_client.upgraded_to_websocket();
                return Ok(HttpResponse::WebSocketUpgrade {
                    stream: WebSocketUpgradeStream::SshChannel(stream),
                    response,
                    disconnection,
                });
            }
        }
    }
}
