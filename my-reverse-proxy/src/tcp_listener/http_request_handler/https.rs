use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http::StatusCode;
use http_body_util::{combinators::BoxBody, BodyExt, Full};

use crate::{
    configurations::HttpListenPortConfiguration,
    http_proxy_pass::{HttpListenPortInfo, HttpProxyPass},
    tcp_listener::https::ClientCertificateData,
    types::ConnectionIp,
};

pub struct HttpsRequestsHandler {
    proxy_passes: Mutex<HashMap<String, Arc<HttpProxyPass>>>,
    connection_ip: ConnectionIp,
    listen_port_config: Arc<HttpListenPortConfiguration>,
    client_certificate: Option<Arc<ClientCertificateData>>,
}

impl HttpsRequestsHandler {
    pub fn new(
        connection_ip: ConnectionIp,
        listen_port_config: Arc<HttpListenPortConfiguration>,
        client_certificate: Option<Arc<ClientCertificateData>>,
    ) -> Self {
        Self {
            proxy_passes: Mutex::new(HashMap::new()),
            connection_ip,
            listen_port_config,
            client_certificate,
        }
    }

    async fn get_http_proxy_pass(
        &self,
        req: &hyper::Request<hyper::body::Incoming>,
    ) -> Result<Arc<HttpProxyPass>, hyper::Result<hyper::Response<BoxBody<Bytes, String>>>> {
        let host: String = if let Some(host) = req.uri().host() {
            host.to_string()
        } else if let Some(host) = req
            .headers()
            .get(hyper::header::HOST)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(':').next().unwrap_or(v).trim())
            .filter(|v| !v.is_empty())
        {
            // Origin-form HTTP/1.1 request (e.g. an http/1.1 client reaching this
            // h2-typed endpoint via ALPN fallback): the authority is only in the
            // `Host` header, not the request URI.
            host.to_string()
        } else {
            crate::app::APP_CTX.proxy_logs.write_port(
                self.listen_port_config.listen_host.get_log_key().as_str(),
                self.connection_ip.get_ip_log(),
                format!(
                    "Rejected request: can not detect host. Uri:{}. Headers: {:?}",
                    req.uri(),
                    req.headers()
                ),
            );
            return Err(create_err_response(
                StatusCode::BAD_REQUEST,
                "Unknown host".to_string().into_bytes(),
            ));
        };

        let host_key = host.to_ascii_lowercase();

        {
            let map = self.proxy_passes.lock().unwrap();
            if let Some(existing) = map.get(&host_key) {
                return Ok(existing.clone());
            }
        }

        let http_endpoint_info = self
            .listen_port_config
            .get_http_endpoint_info(Some(host.as_str()));
        let Some(http_endpoint_info) = http_endpoint_info else {
            crate::app::APP_CTX.proxy_logs.write_port(
                self.listen_port_config.listen_host.get_log_key().as_str(),
                self.connection_ip.get_ip_log(),
                format!(
                    "Rejected request: no endpoint configured for host [{}]",
                    host
                ),
            );
            let content =
                crate::error_templates::generate_layout(400, "No configuration found", None);
            return Err(create_err_response(StatusCode::BAD_REQUEST, content));
        };

        // mTLS is enforced only during the TLS handshake, which is bound to the
        // SNI used to open this connection. A browser may coalesce HTTP/2
        // requests for several hosts onto one connection (RFC 7540 §9.1.1), and a
        // hostile client can simply send a different `:authority`. If the
        // resolved endpoint requires a client certificate but this connection
        // presented none, refuse with 421 Misdirected Request so the client
        // re-opens a dedicated connection (with the right SNI) and performs mTLS.
        if !http_endpoint_info.connection_satisfies_client_cert(self.client_certificate.is_some()) {
            crate::app::APP_CTX.proxy_logs.write_port(
                self.listen_port_config.listen_host.get_log_key().as_str(),
                self.connection_ip.get_ip_log(),
                format!(
                    "Rejected request for host [{}]: endpoint requires a client certificate but the TLS connection presented none (HTTP/2 coalescing or SNI mismatch)",
                    host
                ),
            );
            let content = crate::error_templates::generate_layout(
                421,
                "Misdirected Request",
                Some("A client certificate is required for this host".into()),
            );
            return Err(create_err_response(
                StatusCode::MISDIRECTED_REQUEST,
                content,
            ));
        }

        // Enforce the *request-host* endpoint's IP allow-list per request. The
        // connection-level check in handle_connection only sees the SNI
        // endpoint; a coalesced / cross-`Host` request (RFC 7540 §9.1.1) must
        // still satisfy the allow-list of the vhost it actually targets. Skip
        // when the client IP is unknown (unix socket), matching the
        // location-level check in HttpProxyPass::send_payload.
        if let Some(ip_list_id) = http_endpoint_info.whitelisted_ip_list_id.as_ref() {
            if let Some(client_ip) = self.connection_ip.get_ip_addr() {
                let is_whitelisted = crate::app::APP_CTX
                    .current_configuration
                    .get(|config| {
                        config
                            .white_list_ip_list
                            .is_white_listed(ip_list_id, &client_ip)
                    })
                    .await;
                if !is_whitelisted {
                    crate::app::APP_CTX.proxy_logs.write_port(
                        self.listen_port_config.listen_host.get_log_key().as_str(),
                        self.connection_ip.get_ip_log(),
                        format!(
                            "Rejected request for host [{}]: client IP is not in the endpoint allow-list",
                            host
                        ),
                    );
                    let content =
                        crate::error_templates::generate_layout(401, "Restricted by IP", None);
                    return Err(create_err_response(StatusCode::UNAUTHORIZED, content));
                }
            }
        }

        if crate::app::APP_CTX
            .debug_flags
            .is_endpoint_debug(http_endpoint_info.host_endpoint.as_str())
        {
            crate::app::APP_CTX.proxy_logs.write(
                http_endpoint_info.host_endpoint.as_str(),
                None,
                self.connection_ip.get_ip_log(),
                format!("Detected. [{}]{:?}", req.method(), req.uri()),
            );
        }

        let listening_port_info = HttpListenPortInfo {
            endpoint_type: http_endpoint_info.listen_endpoint_type,
            listen_host: self.listen_port_config.listen_host.clone(),
        };

        let http_proxy_pass = HttpProxyPass::new(
            self.connection_ip,
            http_endpoint_info,
            listening_port_info,
            self.client_certificate.clone(),
        )
        .await;

        let http_proxy_pass = Arc::new(http_proxy_pass);

        {
            let mut map = self.proxy_passes.lock().unwrap();
            if let Some(existing) = map.get(&host_key) {
                return Ok(existing.clone());
            }
            map.insert(host_key, http_proxy_pass.clone());
        }

        Ok(http_proxy_pass)
    }

    pub async fn handle_request(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
        match self.get_http_proxy_pass(&req).await {
            Ok(proxy_pass) => {
                super::handle_requests::handle_requests(req, &proxy_pass, self.connection_ip).await
            }
            Err(err) => err,
        }
    }

    pub async fn dispose(&self) {
        let proxy_passes: Vec<Arc<HttpProxyPass>> = {
            let mut map = self.proxy_passes.lock().unwrap();
            map.drain().map(|(_, v)| v).collect()
        };
        for proxy_pass in proxy_passes {
            proxy_pass.dispose().await;
        }
    }
}

pub async fn handle_request(
    request_handler: Arc<HttpsRequestsHandler>,
    req: hyper::Request<hyper::body::Incoming>,
) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
    request_handler.handle_request(req).await
}

fn create_err_response(
    status_code: StatusCode,
    content: impl Into<Bytes>,
) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
    let result = hyper::Response::builder()
        .status(status_code)
        .body(
            Full::new(content.into())
                .map_err(|e| crate::to_hyper_error(e))
                .boxed(),
        )
        .unwrap();

    Ok(result)
}
