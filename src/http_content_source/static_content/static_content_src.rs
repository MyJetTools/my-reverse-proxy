use std::sync::Arc;

use my_http_server::WebContentType;

use crate::configurations::*;

use crate::http_proxy_pass::{content_source::HttpResponse, ProxyPassError};

use super::super::{RequestExecutor, RequestExecutorResult};

pub struct StaticContentSrc {
    pub inner: Arc<StaticContentExecutor>,
}

impl StaticContentSrc {
    pub fn new(config: Arc<StaticContentConfig>) -> Self {
        Self {
            inner: Arc::new(StaticContentExecutor { config }),
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
    config: Arc<StaticContentConfig>,
}

#[async_trait::async_trait]
impl RequestExecutor for StaticContentExecutor {
    async fn execute_request(&self) -> Result<RequestExecutorResult, ProxyPassError> {
        Ok(RequestExecutorResult {
            status_code: self.config.status_code,
            content_type: if let Some(content_type) = self.config.content_type.clone() {
                Some(WebContentType::Raw(content_type))
            } else {
                None
            },
            body: self.config.body.clone(),
        })
    }
}
