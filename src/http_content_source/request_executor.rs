use crate::http_proxy_pass::ProxyPassError;

use super::WebContentType;

pub struct RequestExecutorResult {
    pub status_code: u16,
    pub content_type: Option<WebContentType>,
    pub body: Vec<u8>,
}

#[async_trait::async_trait]
pub trait RequestExecutor {
    async fn execute_request(&self) -> Result<RequestExecutorResult, ProxyPassError>;
}
