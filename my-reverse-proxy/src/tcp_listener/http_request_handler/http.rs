use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http::StatusCode;
use http_body_util::{combinators::BoxBody, BodyExt, Full};

use crate::{configurations::HttpListenPortConfiguration, http_proxy_pass::*, types::ConnectionIp};

pub struct HttpRequestHandler {
    proxy_passes: Mutex<HashMap<String, Arc<HttpProxyPass>>>,
    connection_ip: ConnectionIp,
    listen_port_config: Arc<HttpListenPortConfiguration>,
}

impl HttpRequestHandler {
    pub fn new(
        connection_ip: ConnectionIp,
        listen_port_config: Arc<HttpListenPortConfiguration>,
    ) -> Self {
        Self {
            proxy_passes: Mutex::new(HashMap::new()),
            connection_ip,
            listen_port_config,
        }
    }

    async fn get_http_proxy_pass(
        &self,
        req: &hyper::Request<hyper::body::Incoming>,
    ) -> Result<Arc<HttpProxyPass>, hyper::Result<hyper::Response<BoxBody<Bytes, String>>>> {
        let Some(host) = req.uri().host() else {
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

        let http_endpoint_info = self.listen_port_config.get_http_endpoint_info(Some(host));
        if http_endpoint_info.is_none() {
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
        }

        let http_endpoint_info = http_endpoint_info.unwrap();

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
            None,
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
    request_handler: Arc<HttpRequestHandler>,
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
