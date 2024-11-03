use std::sync::Arc;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use tokio::sync::Mutex;

use crate::{app::AppContext, configurations::*, http_server::ClientCertificateData};

use super::{
    GoogleAuthResult, HttpProxyPassIdentity, HttpProxyPassInner, HttpRequestBuilder,
    ProxyPassError, ProxyPassLocations,
};

pub struct HttpProxyPass {
    pub inner: Mutex<HttpProxyPassInner>,
    pub listening_port_info: HttpListenPortInfo,
    pub endpoint_info: Arc<HttpEndpointInfo>,
}

impl HttpProxyPass {
    pub fn new(
        endpoint_info: Arc<HttpEndpointInfo>,
        listening_port_info: HttpListenPortInfo,
        client_cert: Option<ClientCertificateData>,
    ) -> Self {
        let locations = ProxyPassLocations::new(&endpoint_info);
        Self {
            inner: Mutex::new(HttpProxyPassInner::new(
                HttpProxyPassIdentity::new(client_cert),
                locations,
                listening_port_info.clone(),
            )),

            listening_port_info,
            endpoint_info,
        }
    }

    pub async fn send_payload(
        &self,
        app: &Arc<AppContext>,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Result<hyper::Response<BoxBody<Bytes, String>>>, ProxyPassError> {
        if self.endpoint_info.debug {
            println!(
                "Request: {}. Uri: {}. Headers{:?}",
                self.endpoint_info.host_endpoint.as_str(),
                req.uri(),
                req.headers()
            );
        }

        let mut req = HttpRequestBuilder::new(self.endpoint_info.http_type.clone(), req);

        let (build_result, content_source) = {
            let mut inner = self.inner.lock().await;

            match self.handle_auth_with_g_auth(app, &req).await {
                GoogleAuthResult::Passed(user) => inner.identity.ga_user = user,
                GoogleAuthResult::Content(content) => return Ok(content),
                GoogleAuthResult::DomainIsNotAuthorized => {
                    return Err(ProxyPassError::Unauthorized);
                }
            }

            if let Some(allowed_users) = self.endpoint_info.allowed_user_list.as_ref() {
                if let Some(identity) = inner.identity.get_identity() {
                    if !allowed_users.is_allowed(identity) {
                        return Err(ProxyPassError::UserIsForbidden);
                    }
                }
            }

            let build_result = req.populate_and_build(self, &inner).await?;

            let proxy_pass_location = inner.locations.find_mut(build_result.get_location_index());

            if !proxy_pass_location
                .config
                .whitelisted_ip
                .is_whitelisted(&self.listening_port_info.socket_addr.ip())
            {
                return Err(ProxyPassError::IpRestricted(
                    self.listening_port_info.socket_addr.ip().to_string(),
                ));
            }

            (
                build_result,
                proxy_pass_location.connect_if_require(app).await?,
            )
        };

        let request = req.get();

        let (location_index, mut response) = match build_result {
            super::BuildResult::HttpRequest(location_index) => {
                (location_index, content_source.send_request(request).await?)
            }
            super::BuildResult::WebSocketUpgrade {
                location_index,
                upgrade_response,
                web_socket,
            } => {
                if self.endpoint_info.debug {
                    println!("Doing web_socket upgrade");
                }

                let (channel, _) = content_source.upgrade_websocket(request.clone()).await?;

                // let upgraded = hyper::upgrade::on(request).await?;

                if let Some(web_socket) = web_socket.lock().await.take() {
                    match channel {
                        super::WebSocketUpgradeStream::TcpStream(tcp_stream) => {
                            tokio::spawn(super::web_socket_loop(
                                web_socket,
                                tcp_stream,
                                self.endpoint_info.debug,
                            ));
                        }

                        super::WebSocketUpgradeStream::TlsStream(tcp_stream) => {
                            tokio::spawn(super::web_socket_loop(
                                web_socket,
                                tcp_stream,
                                self.endpoint_info.debug,
                            ));
                        }
                        super::WebSocketUpgradeStream::SshChannel(async_channel) => {
                            tokio::spawn(super::web_socket_loop(
                                web_socket,
                                async_channel,
                                self.endpoint_info.debug,
                            ));
                        }
                    }
                }

                let response = my_http_client::utils::into_full_body_response(upgrade_response);
                (location_index, response)
            }
        };

        let inner = self.inner.lock().await;
        super::http_response_builder::modify_resp_headers(
            self,
            &inner,
            &req,
            response.headers_mut(),
            &location_index,
        );

        return Ok(Ok(response));
    }

    pub async fn dispose(&self) {
        let mut inner = self.inner.lock().await;
        inner.disposed = true;
    }
}
