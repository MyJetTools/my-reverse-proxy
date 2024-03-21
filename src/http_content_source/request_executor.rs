use crate::http_proxy_pass::ProxyPassError;

use super::WebContentType;

#[async_trait::async_trait]
pub trait RequestExecutor {
    async fn execute_request(
        &self,
    ) -> Result<Option<(Vec<u8>, Option<WebContentType>)>, ProxyPassError>;
}
