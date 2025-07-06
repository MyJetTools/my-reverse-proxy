use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::combinators::BoxBody;

use crate::http_proxy_pass::HttpProxyPass;

pub struct HttpsRequestsHandler {
    proxy_pass: HttpProxyPass,
    socket_addr: SocketAddr,
}

impl HttpsRequestsHandler {
    pub fn new(proxy_pass: HttpProxyPass, socket_addr: SocketAddr) -> Self {
        Self {
            proxy_pass,
            socket_addr,
        }
    }

    pub async fn handle_request(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
        super::handle_requests::handle_requests(req, &self.proxy_pass, &self.socket_addr).await
    }

    pub async fn dispose(&self) {
        self.proxy_pass.dispose().await;
    }
}

pub async fn handle_request(
    request_handler: Arc<HttpsRequestsHandler>,
    req: hyper::Request<hyper::body::Incoming>,
) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
    request_handler.handle_request(req).await
}
