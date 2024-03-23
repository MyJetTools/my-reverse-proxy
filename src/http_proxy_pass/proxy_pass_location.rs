use hyper::Uri;

use crate::{app::AppContext, settings::ModifyHttpHeadersSettings};

use super::{HttpProxyPassContentSource, ProxyPassError};

pub struct ProxyPassLocation {
    pub path: String,
    pub id: i64,
    pub modify_headers: Option<ModifyHttpHeadersSettings>,
    pub content_source: HttpProxyPassContentSource,
}

impl ProxyPassLocation {
    pub fn new(
        id: i64,
        path: String,
        modify_headers: Option<ModifyHttpHeadersSettings>,
        content_source: HttpProxyPassContentSource,
    ) -> Self {
        Self {
            path,
            id,
            modify_headers,
            content_source,
        }
    }
    pub fn is_my_uri(&self, uri: &Uri) -> bool {
        let result = rust_extensions::str_utils::starts_with_case_insensitive(
            uri.path(),
            self.path.as_str(),
        );

        result
    }

    pub fn is_http1(&self) -> bool {
        match &self.content_source {
            HttpProxyPassContentSource::Http(remote_http_location) => {
                remote_http_location.remote_endpoint.is_http1()
            }
            HttpProxyPassContentSource::LocalPath(_) => {
                panic!("LocalPath can not return is_http1")
            }
            HttpProxyPassContentSource::PathOverSsh(_) => {
                panic!("PathOverSsh can not return is_http1")
            }
        }
    }

    pub async fn connect_if_require(&mut self, app: &AppContext) -> Result<(), ProxyPassError> {
        match &mut self.content_source {
            HttpProxyPassContentSource::Http(remote_http_location) => {
                return remote_http_location.connect_if_require(app).await;
            }

            HttpProxyPassContentSource::LocalPath(_) => return Ok(()),
            HttpProxyPassContentSource::PathOverSsh(file_over_ssh) => {
                return file_over_ssh.connect_if_require(app).await;
            }
        }
    }
}
