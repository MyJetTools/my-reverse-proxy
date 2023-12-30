use http_body_util::Full;
use hyper::{body::Bytes, client::conn::http1::SendRequest, Uri};

use super::{HttpClientConnection, HttpClientError};

pub struct HttpClient {
    pub connection: Option<HttpClientConnection>,
}

impl HttpClient {
    pub fn new() -> Self {
        Self { connection: None }
    }

    pub async fn connect(proxy_pass: &Uri) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
        let is_https = super::utils::is_https(proxy_pass);
        if is_https {
            super::connect_to_tls_endpoint(proxy_pass).await
        } else {
            super::connect_to_http_endpoint(proxy_pass).await
        }
    }
}
