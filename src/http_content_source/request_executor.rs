use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt};

use crate::http_proxy_pass::ProxyPassError;

use super::WebContentType;

pub struct RequestExecutorResult {
    pub status_code: u16,
    pub content_type: Option<WebContentType>,
    pub body: Vec<u8>,
}

impl Into<hyper::Response<BoxBody<Bytes, String>>> for RequestExecutorResult {
    fn into(self) -> hyper::Response<BoxBody<Bytes, String>> {
        let mut builder = hyper::Response::builder().status(self.status_code);

        if let Some(content_type) = self.content_type {
            builder = builder.header("Content-Type", content_type.as_str());
        }

        let full_body = http_body_util::Full::new(hyper::body::Bytes::from(self.body));
        builder
            .body(full_body.map_err(|e| crate::to_hyper_error(e)).boxed())
            .unwrap()
    }
}
#[async_trait::async_trait]
pub trait RequestExecutor {
    async fn execute_request(&self) -> Result<RequestExecutorResult, ProxyPassError>;
}
