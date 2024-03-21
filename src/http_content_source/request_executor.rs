use crate::http_proxy_pass::ProxyPassError;

#[async_trait::async_trait]
pub trait RequestExecutor {
    async fn execute_request(&self) -> Result<Option<Vec<u8>>, ProxyPassError>;
}
