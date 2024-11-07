use std::sync::Arc;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use my_http_client::utils::into_full_body_response;
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

        let req = HttpRequestBuilder::new(self.endpoint_info.http_type.clone(), req);

        let (request, content_source) = {
            let mut inner = self.inner.lock().await;

            match self.handle_auth_with_g_auth(app, &req).await {
                GoogleAuthResult::Passed(user) => inner.identity.ga_user = user,
                GoogleAuthResult::Content(content) => return Ok(content),
                GoogleAuthResult::DomainIsNotAuthorized => {
                    return Err(ProxyPassError::Unauthorized);
                }
            }

            let request = req.into_response(self, &inner).await?;

            if let Some(allowed_users) = self.endpoint_info.allowed_user_list.as_ref() {
                if let Some(identity) = inner.identity.get_identity() {
                    if !allowed_users.is_allowed(identity) {
                        return Err(ProxyPassError::UserIsForbidden);
                    }
                }
            }

            let proxy_pass_location = inner.locations.find_mut(&request.location_index);

            if !proxy_pass_location
                .config
                .whitelisted_ip
                .is_whitelisted(&self.listening_port_info.socket_addr.ip())
            {
                return Err(ProxyPassError::IpRestricted(
                    self.listening_port_info.socket_addr.ip().to_string(),
                ));
            }

            (request, proxy_pass_location.connect_if_require(app).await?)
        };

        let result = content_source
            .send_request(request.request, crate::consts::DEFAULT_HTTP_REQUEST_TIMEOUT)
            .await?;

        let mut response = match result {
            super::HttpResponse::Response(response) => response,
            super::HttpResponse::WebSocketUpgrade {
                stream,
                response,
                disconnection,
            } => match stream {
                super::WebSocketUpgradeStream::TcpStream(tcp_stream) => {
                    if let Some(web_socket_upgrade) = request.web_socket_upgrade {
                        let server_web_socket = web_socket_upgrade.server_web_socket;
                        tokio::spawn(super::start_web_socket_loop(
                            server_web_socket,
                            tcp_stream,
                            self.endpoint_info.debug,
                            disconnection,
                        ));

                        into_full_body_response(web_socket_upgrade.upgrade_response)
                    } else {
                        response
                    }
                }
                super::WebSocketUpgradeStream::TlsStream(tls_stream) => {
                    if let Some(web_socket_upgrade) = request.web_socket_upgrade {
                        let server_web_socket = web_socket_upgrade.server_web_socket;
                        tokio::spawn(super::start_web_socket_loop(
                            server_web_socket,
                            tls_stream,
                            self.endpoint_info.debug,
                            disconnection,
                        ));

                        into_full_body_response(web_socket_upgrade.upgrade_response)
                    } else {
                        response
                    }
                }
                super::WebSocketUpgradeStream::SshChannel(async_channel) => {
                    if let Some(web_socket_upgrade) = request.web_socket_upgrade {
                        let server_web_socket = web_socket_upgrade.server_web_socket;
                        tokio::spawn(super::start_web_socket_loop(
                            server_web_socket,
                            async_channel,
                            self.endpoint_info.debug,
                            disconnection,
                        ));

                        into_full_body_response(web_socket_upgrade.upgrade_response)
                    } else {
                        response
                    }
                }
            },
        };

        let inner = self.inner.lock().await;
        super::http_response_builder::modify_resp_headers(
            self,
            &inner,
            &request.req_parts,
            response.headers_mut(),
            &request.location_index,
        );

        return Ok(Ok(response));
    }

    pub async fn dispose(&self) {
        let mut inner = self.inner.lock().await;
        inner.disposed = true;
    }
}
