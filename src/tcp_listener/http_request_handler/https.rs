use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::combinators::BoxBody;

use crate::{app::AppContext, http_proxy_pass::HttpProxyPass};

pub struct HttpsRequestsHandler {
    proxy_pass: HttpProxyPass,
    app: Arc<AppContext>,
    socket_addr: SocketAddr,
}

impl HttpsRequestsHandler {
    pub fn new(app: Arc<AppContext>, proxy_pass: HttpProxyPass, socket_addr: SocketAddr) -> Self {
        app.http_connections
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            proxy_pass,
            app,
            socket_addr,
        }
    }

    pub async fn handle_request(
        &self,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
        super::handle_requests::handle_requests(&self.app, req, &self.proxy_pass, &self.socket_addr)
            .await
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
