use hyper::Uri;

use crate::{app::AppContext, settings::ModifyHttpHeadersSettings, types::WhiteListedIpList};

use super::{HttpProxyPassContentSource, ProxyPassError};

pub struct ProxyPassLocation {
    pub path: String,
    pub id: i64,
    pub modify_headers: Option<ModifyHttpHeadersSettings>,
    pub content_source: HttpProxyPassContentSource,
    pub whitelisted_ip: WhiteListedIpList,
}

impl ProxyPassLocation {
    pub fn new(
        id: i64,
        path: String,
        modify_headers: Option<ModifyHttpHeadersSettings>,
        content_source: HttpProxyPassContentSource,
        whitelisted_ip: WhiteListedIpList,
    ) -> Self {
        Self {
            path,
            id,
            modify_headers,
            content_source,
            whitelisted_ip,
        }
    }

    pub fn is_my_uri(&self, uri: &Uri) -> bool {
        let result = rust_extensions::str_utils::starts_with_case_insensitive(
            uri.path(),
            self.path.as_str(),
        );

        result
    }

    pub fn is_http1(&self) -> Option<bool> {
        match &self.content_source {
            HttpProxyPassContentSource::Http(remote_http_location) => {
                Some(remote_http_location.remote_endpoint.is_http1())
            }
            HttpProxyPassContentSource::LocalPath(_) => None,
            HttpProxyPassContentSource::PathOverSsh(_) => None,
            HttpProxyPassContentSource::Static(_) => None,
        }
    }

    pub async fn connect_if_require(
        &mut self,
        app: &AppContext,
        debug: bool,
    ) -> Result<(), ProxyPassError> {
        match &mut self.content_source {
            HttpProxyPassContentSource::Http(remote_http_location) => {
                return remote_http_location.connect_if_require(app, debug).await;
            }

            HttpProxyPassContentSource::LocalPath(_) => return Ok(()),
            HttpProxyPassContentSource::PathOverSsh(file_over_ssh) => {
                return file_over_ssh.connect_if_require(app).await;
            }
            HttpProxyPassContentSource::Static(_) => return Ok(()),
        }
    }
}
