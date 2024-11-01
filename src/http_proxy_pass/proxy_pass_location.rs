use std::sync::Arc;

use hyper::Uri;

use crate::{
    app::AppContext, configurations::*, http_client::HTTP_CLIENT_TIMEOUT,
    http_proxy_pass::HttpProxyPassContentSource,
};

use super::ProxyPassError;

pub struct ProxyPassLocation {
    pub content_source: Option<Arc<HttpProxyPassContentSource>>,
    pub config: Arc<ProxyPassLocationConfig>,
    pub compress: bool,
    pub debug: bool,
}

impl ProxyPassLocation {
    pub fn new(config: Arc<ProxyPassLocationConfig>, debug: bool, compress: bool) -> Self {
        //let content_source = config.create_content_source(debug, request_timeout);
        //let is_http1 = content_source.is_http1();
        Self {
            content_source: None,
            config,
            compress,
            debug,
        }
    }

    pub fn is_my_uri(&self, uri: &Uri) -> bool {
        let result = rust_extensions::str_utils::starts_with_case_insensitive(
            uri.path(),
            self.config.path.as_str(),
        );

        result
    }

    pub fn is_http1(&self) -> Option<bool> {
        self.config.is_http1()
    }

    pub async fn connect_if_require(
        &mut self,
        app: &AppContext,
    ) -> Result<Arc<HttpProxyPassContentSource>, ProxyPassError> {
        if let Some(content_source) = self.content_source.as_ref() {
            return Ok(content_source.clone());
        }

        let client_source = self
            .config
            .create_and_connect(app, self.debug, HTTP_CLIENT_TIMEOUT)
            .await?;

        //let content_source = HttpProxyPassContentSource::connect(app, &self.config).await?;

        let result = Arc::new(client_source);
        self.content_source = Some(result.clone());

        Ok(result)
    }
}
