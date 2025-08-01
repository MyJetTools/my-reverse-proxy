use std::sync::Arc;

use my_http_server::WebContentType;

use crate::http_proxy_pass::{content_source::HttpResponse, ProxyPassError};

use super::{RequestExecutor, RequestExecutorResult};

pub struct StaticContentSrc {
    pub inner: Arc<StaticContentExecutor>,
}

impl StaticContentSrc {
    pub fn new(status_code: u16, content_type: Option<String>, body: Vec<u8>) -> Self {
        Self {
            inner: Arc::new(StaticContentExecutor {
                status_code,
                content_type,
                body,
            }),
        }
    }

    pub fn get_request_executor(
        &self,
    ) -> Result<Arc<dyn RequestExecutor + Send + Sync + 'static>, ProxyPassError> {
        Ok(self.inner.clone())
    }

    pub async fn execute(
        &self,
        _req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        println!("Executing static request");
        let request_executor = self.get_request_executor()?;
        let result = request_executor.execute_request().await?;
        Ok(HttpResponse::Response(result.into()))
    }
}

pub struct StaticContentExecutor {
    pub status_code: u16,
    pub body: Vec<u8>,
    content_type: Option<String>,
}

#[async_trait::async_trait]
impl RequestExecutor for StaticContentExecutor {
    async fn execute_request(&self) -> Result<RequestExecutorResult, ProxyPassError> {
        Ok(RequestExecutorResult {
            status_code: self.status_code,
            content_type: if let Some(content_type) = self.content_type.clone() {
                Some(WebContentType::Raw(content_type))
            } else {
                None
            },
            body: self.body.clone(),
        })
    }
}
