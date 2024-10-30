use std::{sync::Arc, time::Duration};

use hyper::Uri;

use crate::{app::AppContext, configurations::*, http_proxy_pass::HttpProxyPassContentSource};

use super::ProxyPassError;

pub struct ProxyPassLocation {
    pub content_source: HttpProxyPassContentSource,
    pub config: Arc<ProxyPassLocationConfig>,
    pub is_http1: Option<bool>,
}

impl ProxyPassLocation {
    pub fn new(
        config: Arc<ProxyPassLocationConfig>,
        debug: bool,
        request_timeout: Duration,
    ) -> Self {
        let content_source = config.create_content_source(debug, request_timeout);
        let is_http1 = content_source.is_http1();
        Self {
            content_source: content_source,
            config,
            is_http1,
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
        self.is_http1
    }

    pub async fn connect_if_require(
        &mut self,
        app: &AppContext,
        debug: bool,
    ) -> Result<(), ProxyPassError> {
        self.content_source
            .connect_if_require(app, &self.config.domain_name, debug)
            .await
    }

    pub async fn reconnect(&mut self, app: &AppContext, debug: bool) -> Result<(), ProxyPassError> {
        self.content_source.disconnect();
        self.connect_if_require(app, debug).await
    }
}
