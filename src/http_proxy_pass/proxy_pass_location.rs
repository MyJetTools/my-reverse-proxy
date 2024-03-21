use hyper::Uri;

use crate::{
    app::AppContext,
    http_content_source::{FileContentSrc, RemoteHttpContentSource},
    settings::ModifyHttpHeadersSettings,
};

use super::ProxyPassError;

pub enum ProxyPassContentSource {
    Http(RemoteHttpContentSource),
    File(FileContentSrc),
}

pub struct ProxyPassLocation {
    pub path: String,
    pub id: i64,
    pub modify_headers: Option<ModifyHttpHeadersSettings>,
    pub content_source: ProxyPassContentSource,
}

impl ProxyPassLocation {
    pub fn new(
        id: i64,
        path: String,
        modify_headers: Option<ModifyHttpHeadersSettings>,
        content_source: ProxyPassContentSource,
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

    pub async fn connect_if_require(&mut self, app: &AppContext) -> Result<(), ProxyPassError> {
        match &mut self.content_source {
            ProxyPassContentSource::Http(remote_http_location) => {
                return remote_http_location.connect_if_require(app).await;
            }
            ProxyPassContentSource::File(_) => return Ok(()),
        }
    }
}
