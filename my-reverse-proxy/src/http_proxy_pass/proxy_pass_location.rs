use std::sync::Arc;

use hyper::Uri;

use crate::{configurations::*, http_proxy_pass::content_source::*};

pub struct ProxyPassLocation {
    pub content_source: Arc<HttpProxyPassContentSource>,
    pub config: Arc<ProxyPassLocationConfig>,
    pub compress: bool,
    pub debug: bool,
    pub trace_payload: bool,
}

impl ProxyPassLocation {
    pub async fn new(
        config: Arc<ProxyPassLocationConfig>,
        debug: bool,
        compress: bool,
        trace_payload: bool,
    ) -> Self {
        //let content_source = config.create_content_source(debug, request_timeout);
        //let is_http1 = content_source.is_http1();

        let content_source = config
            .create_data_source(debug, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT)
            .await;
        let result = Self {
            content_source: Arc::new(content_source),
            config,
            compress,
            debug,
            trace_payload,
        };

        result
    }

    pub fn is_my_uri(&self, uri: &Uri) -> bool {
        let result = rust_extensions::str_utils::starts_with_case_insensitive(
            uri.path(),
            self.config.path.as_str(),
        );

        result
    }

    pub fn is_http1(&self) -> Option<bool> {
        self.config.is_remote_content_http1()
    }

    /*
    pub async fn connect_if_require(
        &mut self,
        app: &AppContext,
    ) -> Result<Arc<HttpProxyPassContentSource>, ProxyPassError> {
        if let Some(content_source) = self.content_source.as_ref() {
            return Ok(content_source.clone());
        }

        //let content_source = HttpProxyPassContentSource::connect(app, &self.config).await?;

        let result = Arc::new(client_source);
        self.content_source = Some(result.clone());

        Ok(result)
    }
     */
}
