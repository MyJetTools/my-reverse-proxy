use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http::StatusCode;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use tokio::sync::Mutex;

use crate::{
    configurations::HttpListenPortConfiguration,
    http_proxy_pass::{HostPort, HttpListenPortInfo, HttpProxyPass},
};

pub struct HttpRequestHandler {
    proxy_pass: Mutex<Option<Arc<HttpProxyPass>>>,
    socket_addr: SocketAddr,
    listen_port_config: Arc<HttpListenPortConfiguration>,
}

impl HttpRequestHandler {
    pub fn new(
        socket_addr: SocketAddr,
        listen_port_config: Arc<HttpListenPortConfiguration>,
    ) -> Self {
        Self {
            proxy_pass: Mutex::new(None),
            socket_addr,
            listen_port_config,
        }
    }

    async fn get_http_proxy_pass(
        &self,
        req: &hyper::Request<hyper::body::Incoming>,
    ) -> Result<Arc<HttpProxyPass>, hyper::Result<hyper::Response<BoxBody<Bytes, String>>>> {
        let mut write_access = self.proxy_pass.lock().await;

        if let Some(proxy_pass) = write_access.clone() {
            return Ok(proxy_pass);
        }

        let host = req.get_host();

        if host.is_none() {
            println!(
                "Can not detect host. Uri:{}. Headers: {:?}",
                req.uri(),
                req.headers()
            );
            return Err(create_err_response(
                StatusCode::BAD_REQUEST,
                "Unknown host".to_string().into_bytes(),
            ));
        }
        let http_endpoint_info = self
            .listen_port_config
            .get_http_endpoint_info(host.unwrap());
        if http_endpoint_info.is_none() {
            let content = super::utils::generate_layout(400, "No configuration found", None);
            return Err(create_err_response(StatusCode::BAD_REQUEST, content));
        }

        let http_endpoint_info = http_endpoint_info.unwrap();

        if http_endpoint_info.debug {
            println!(
                "Detected. {}: [{}]{:?}",
                http_endpoint_info.as_str(),
                req.method(),
                req.uri()
            );
        }

        let listening_port_info = HttpListenPortInfo {
            endpoint_type: http_endpoint_info.listen_endpoint_type,
            socket_addr: self.socket_addr,
        };

        let http_proxy_pass =
            HttpProxyPass::new(http_endpoint_info, listening_port_info, None).await;

        let http_proxy_pass = Arc::new(http_proxy_pass);

        *write_access = Some(http_proxy_pass.clone());

        Ok(http_proxy_pass)
    }

    pub async fn handle_request(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
        match self.get_http_proxy_pass(&req).await {
            Ok(proxy_pass) => {
                super::handle_requests::handle_requests(req, &proxy_pass, &self.socket_addr).await
            }
            Err(err) => err,
        }
    }

    pub async fn dispose(&self) {
        let proxy_pass = self.proxy_pass.lock().await.take();

        if let Some(proxy_pass) = proxy_pass {
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
