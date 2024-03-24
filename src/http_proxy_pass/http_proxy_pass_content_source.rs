use crate::http_content_source::{
    LocalPathContentSrc, PathOverSshContentSource, RemoteHttpContentSource, StaticContentSrc,
};

pub enum HttpProxyPassContentSource {
    Http(RemoteHttpContentSource),
    LocalPath(LocalPathContentSrc),
    PathOverSsh(PathOverSshContentSource),
    Static(StaticContentSrc),
}
