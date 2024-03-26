use crate::http_content_source::{
    LocalPathContentSrc, PathOverSshContentSource, RemoteHttpContentSource, StaticContentSrc,
};

pub enum HttpProxyPassContentSource {
    Http(RemoteHttpContentSource),
    LocalPath(LocalPathContentSrc),
    PathOverSsh(PathOverSshContentSource),
    Static(StaticContentSrc),
}

impl HttpProxyPassContentSource {
    pub fn to_string(&self) -> String {
        match self {
            HttpProxyPassContentSource::Http(remote_http_location) => {
                return format!("HttpProxyPass: {:?}", remote_http_location.remote_endpoint);
            }
            HttpProxyPassContentSource::LocalPath(local_path) => {
                return format!("LocalPath: {}", local_path.file_path);
            }
            HttpProxyPassContentSource::PathOverSsh(path_over_ssh) => {
                return format!("PathOverSsh: {}", path_over_ssh.file_path);
            }
            HttpProxyPassContentSource::Static(static_content) => {
                return format!("Static: {}", static_content.status_code);
            }
        }
    }
}
