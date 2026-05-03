use std::sync::Arc;

use arc_swap::ArcSwapOption;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper_util::rt::TokioIo;
use my_http_client::utils::into_full_body_response;
use my_http_client::MyHttpClientDisconnect;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    configurations::*, http_proxy_pass::GoogleAuthResult,
    tcp_listener::https::ClientCertificateData, types::ConnectionIp,
};

use super::{
    HttpListenPortInfo, HttpProxyPassIdentity, HttpProxyPassInner, HttpRequestBuilder,
    ProxyPassError, ProxyPassLocations, WebSocketUpgrade,
};

pub struct HttpProxyPass {
    pub inner: ArcSwapOption<HttpProxyPassInner>,
    pub listening_port_info: HttpListenPortInfo,
    pub endpoint_info: Arc<HttpEndpointInfo>,
}

impl HttpProxyPass {
    pub async fn new(
        connection_ip: ConnectionIp,
        endpoint_info: Arc<HttpEndpointInfo>,
        listening_port_info: HttpListenPortInfo,
        client_cert: Option<Arc<ClientCertificateData>>,
    ) -> Self {
        let locations = ProxyPassLocations::new(&endpoint_info).await;
        Self {
            inner: ArcSwapOption::from(Some(Arc::new(HttpProxyPassInner::new(
                client_cert.map(|itm| HttpProxyPassIdentity::ClientCert(itm)),
                locations,
                listening_port_info.clone(),
                connection_ip,
            )))),

            listening_port_info,
            endpoint_info,
        }
    }

    pub async fn send_payload(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
        connection_ip: ConnectionIp,
        debug: bool,
    ) -> Result<hyper::Result<hyper::Response<BoxBody<Bytes, String>>>, ProxyPassError> {
        if self.endpoint_info.debug {
            println!(
                "Request. Endpoint [{}] Uri: [{}] Headers: {:?}",
                self.endpoint_info.host_endpoint.as_str(),
                req.uri(),
                req.headers()
            );
        }

        let mut req = HttpRequestBuilder::new(self.endpoint_info.listen_endpoint_type.clone(), req);

        let (request, content_source, location_index, trace_payload) = {
            let Some(inner) = self.inner.load_full() else {
                return Err(ProxyPassError::Disposed);
            };

            match self.handle_auth_with_g_auth(&req).await {
                GoogleAuthResult::Passed(user) => {
                    if let Some(email) = user {
                        inner
                            .identity
                            .store(Some(Arc::new(HttpProxyPassIdentity::GoogleUser(email))));
                    }
                }
                GoogleAuthResult::Content(content) => return Ok(content),
                GoogleAuthResult::DomainIsNotAuthorized => {
                    return Err(ProxyPassError::Unauthorized);
                }
            }

            if let Some(allowed_user_list_id) = self.endpoint_info.allowed_user_list_id.as_ref() {
                if let Some(identity) = inner.identity.load_full() {
                    if !crate::app::APP_CTX
                        .allowed_users_list
                        .is_allowed(allowed_user_list_id, identity.as_str())
                        .await
                    {
                        return Err(ProxyPassError::UserIsForbidden);
                    }
                }
            }

            let location_index = inner.locations.find_location_index(req.uri(), debug)?;

            let proxy_pass_location = inner.locations.find(&location_index);

            let trace_payload = proxy_pass_location.trace_payload;

            req.process_headers(self, &inner, proxy_pass_location);

            let request = req.into_request(self, proxy_pass_location).await?;

            if trace_payload {
                println!("Request parts: {:?}", request.req_parts);
            }

            if let Some(ip_addr) = connection_ip.get_ip_addr() {
                if let Some(white_list_ip) = proxy_pass_location.config.ip_white_list_id.as_ref() {
                    if !crate::app::APP_CTX
                        .current_configuration
                        .get(|itm| {
                            itm.white_list_ip_list
                                .is_white_listed(white_list_ip, &ip_addr)
                        })
                        .await
                    {
                        return Err(ProxyPassError::IpRestricted(
                            self.listening_port_info.listen_host.to_string(),
                        ));
                    }
                }
            }

            (
                request,
                proxy_pass_location.content_source.clone(),
                location_index,
                trace_payload,
            )
        };

        let result = content_source.send_request(request.request).await?;

        let mut response = match result {
            super::content_source::HttpResponse::Response(response) => {
                if trace_payload {
                    println!("Response headers: {:?}", response.headers());
                }
                response
            }
            super::content_source::HttpResponse::WebSocketUpgrade {
                stream,
                response,
                disconnection,
            } => {
                if trace_payload {
                    println!(
                        "Response as web-socket upgrade. Headers: {:?}",
                        response.headers()
                    );
                }
                let request_host_for_metric: Option<String> = request
                    .req_parts
                    .headers
                    .get("host")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                    .or_else(|| request.req_parts.uri.host().map(|s| s.to_string()))
                    .map(|s| {
                        let host_no_port = match s.find(':') {
                            Some(idx) => &s[..idx],
                            None => s.as_str(),
                        };
                        host_no_port.trim().to_string()
                    });
                let domain: Option<String> = self
                    .endpoint_info
                    .tracked_domain(request_host_for_metric.as_deref())
                    .map(str::to_owned);
                match stream {
                    super::content_source::WebSocketUpgradeStream::TcpStream(tcp_stream) => {
                        spawn_websocket_pump(
                            request.web_socket_upgrade,
                            tcp_stream,
                            response,
                            self.endpoint_info.debug,
                            disconnection,
                            trace_payload,
                            domain,
                        )
                    }

                    super::content_source::WebSocketUpgradeStream::UnixStream(unix_stream) => {
                        spawn_websocket_pump(
                            request.web_socket_upgrade,
                            unix_stream,
                            response,
                            self.endpoint_info.debug,
                            disconnection,
                            trace_payload,
                            domain,
                        )
                    }

                    super::content_source::WebSocketUpgradeStream::TlsStream(tls_stream) => {
                        spawn_websocket_pump(
                            request.web_socket_upgrade,
                            tls_stream,
                            response,
                            self.endpoint_info.debug,
                            disconnection,
                            trace_payload,
                            domain,
                        )
                    }
                    super::content_source::WebSocketUpgradeStream::SshChannel(async_channel) => {
                        spawn_websocket_pump(
                            request.web_socket_upgrade,
                            async_channel,
                            response,
                            self.endpoint_info.debug,
                            disconnection,
                            trace_payload,
                            domain,
                        )
                    }
                    super::content_source::WebSocketUpgradeStream::H2Upgraded(h2_upgraded) => {
                        spawn_h2_websocket_pump(
                            request.web_socket_upgrade,
                            h2_upgraded,
                            response,
                            self.endpoint_info.debug,
                            disconnection,
                            domain,
                        )
                    }
                }
            }
        };

        let Some(inner) = self.inner.load_full() else {
            return Err(ProxyPassError::Disposed);
        };

        super::http_response_builder::modify_resp_headers(
            self,
            &inner,
            &request.req_parts,
            response.headers_mut(),
            &location_index,
        );

        return Ok(Ok(response));
    }

    pub async fn dispose(&self) {
        self.inner.store(None);
    }
}

fn spawn_websocket_pump<S>(
    web_socket_upgrade: Option<WebSocketUpgrade>,
    upstream: S,
    fallback_response: hyper::Response<BoxBody<Bytes, String>>,
    debug: bool,
    disconnection: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    trace_payload: bool,
    domain: Option<String>,
) -> hyper::Response<BoxBody<Bytes, String>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    let Some(web_socket_upgrade) = web_socket_upgrade else {
        return fallback_response;
    };

    match web_socket_upgrade {
        WebSocketUpgrade::H1 {
            upgrade_response,
            server_web_socket,
        } => {
            crate::app::spawn_named(
                "ws_pump_h1_start",
                super::start_web_socket_loop(
                    server_web_socket,
                    upstream,
                    debug,
                    disconnection,
                    trace_payload,
                    domain.clone(),
                ),
            );
            into_full_body_response(upgrade_response)
        }
        WebSocketUpgrade::H2 {
            upgrade_response,
            on_upgrade,
        } => {
            crate::app::spawn_named(
                "ws_pump_h2_extended_connect_h1",
                pump_h2_extended_connect(on_upgrade, upstream, debug, disconnection, domain),
            );
            into_full_body_response(upgrade_response)
        }
    }
}

fn spawn_h2_websocket_pump<S>(
    web_socket_upgrade: Option<WebSocketUpgrade>,
    upstream: S,
    fallback_response: hyper::Response<BoxBody<Bytes, String>>,
    debug: bool,
    disconnection: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    domain: Option<String>,
) -> hyper::Response<BoxBody<Bytes, String>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let Some(web_socket_upgrade) = web_socket_upgrade else {
        return fallback_response;
    };

    match web_socket_upgrade {
        WebSocketUpgrade::H2 {
            upgrade_response,
            on_upgrade,
        } => {
            crate::app::spawn_named(
                "ws_pump_h2_extended_connect_h2",
                pump_h2_extended_connect(on_upgrade, upstream, debug, disconnection, domain),
            );
            into_full_body_response(upgrade_response)
        }
        WebSocketUpgrade::H1 { upgrade_response, .. } => {
            if debug {
                println!(
                    "Unexpected h1 WebSocketUpgrade for h2 upstream — returning fallback response"
                );
            }
            into_full_body_response(upgrade_response)
        }
    }
}

async fn pump_h2_extended_connect<S>(
    on_upgrade: hyper::upgrade::OnUpgrade,
    upstream: S,
    debug: bool,
    disconnection: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    domain: Option<String>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let upgraded = match on_upgrade.await {
        Ok(upgraded) => upgraded,
        Err(err) => {
            if debug {
                println!("h2 extended-CONNECT upgrade failed: {:?}", err);
            }
            disconnection.web_socket_disconnect();
            return;
        }
    };

    let upgraded = TokioIo::new(upgraded);

    // Reads on `client_side` are bytes coming FROM the client → upstream (c2s).
    // Reads on `upstream_side` are bytes coming FROM the upstream → client (s2c).
    let copy_result = match domain {
        Some(domain) => {
            let mut client_side = crate::tcp_utils::MeteredStream::new(
                upgraded,
                crate::tcp_utils::WsTrafficRecorder {
                    domain: domain.clone(),
                    direction: crate::tcp_utils::WsDirection::ClientToServer,
                },
            );
            let mut upstream_side = crate::tcp_utils::MeteredStream::new(
                upstream,
                crate::tcp_utils::WsTrafficRecorder {
                    domain,
                    direction: crate::tcp_utils::WsDirection::ServerToClient,
                },
            );
            tokio::io::copy_bidirectional(&mut client_side, &mut upstream_side).await
        }
        None => {
            let mut client_side = upgraded;
            let mut upstream_side = upstream;
            tokio::io::copy_bidirectional(&mut client_side, &mut upstream_side).await
        }
    };

    if debug {
        match copy_result {
            Ok((from_client, from_upstream)) => println!(
                "h2 ext-CONNECT WS pump finished. client->upstream={} bytes, upstream->client={} bytes",
                from_client, from_upstream
            ),
            Err(err) => println!("h2 ext-CONNECT WS pump error: {:?}", err),
        }
    }

    disconnection.web_socket_disconnect();
}
