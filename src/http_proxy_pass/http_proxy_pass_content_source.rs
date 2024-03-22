use crate::http_content_source::{
    LocalPathContentSrc, PathOverSshContentSource, RemoteHttpContentSource,
};

pub enum HttpProxyPassContentSource {
    Http(RemoteHttpContentSource),
    LocalPath(LocalPathContentSrc),
    PathOverSsh(PathOverSshContentSource),
}
