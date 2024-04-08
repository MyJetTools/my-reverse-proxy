use crate::{
    app::AppContext,
    http_content_source::{
        LocalPathContentSrc, PathOverSshContentSource, RemoteHttpContentSource, StaticContentSrc,
    },
};

use super::ProxyPassError;

pub enum HttpProxyPassContentSource {
    Http(RemoteHttpContentSource),
    LocalPath(LocalPathContentSrc),
    PathOverSsh(PathOverSshContentSource),
    Static(StaticContentSrc),
}

impl HttpProxyPassContentSource {
    pub fn is_http1(&self) -> Option<bool> {
        match self {
            Self::Http(remote_http_location) => {
                Some(remote_http_location.remote_endpoint.is_http1())
            }
            Self::LocalPath(_) => None,
            Self::PathOverSsh(_) => None,
            Self::Static(_) => None,
        }
    }

    pub async fn connect_if_require(
        &mut self,
        app: &AppContext,
        debug: bool,
    ) -> Result<(), ProxyPassError> {
        match self {
            Self::Http(remote_http_location) => {
                return remote_http_location.connect_if_require(app, debug).await;
            }

            Self::LocalPath(_) => return Ok(()),
            Self::PathOverSsh(file_over_ssh) => {
                return file_over_ssh.connect_if_require(app).await;
            }
            Self::Static(_) => return Ok(()),
        }
    }
}
