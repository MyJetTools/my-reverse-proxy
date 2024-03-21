use crate::http_content_source::{FileContentSrc, RemoteHttpContentSource, SshFileContentSource};

pub enum ProxyPassContentSource {
    Http(RemoteHttpContentSource),
    File(FileContentSrc),
    FileOverSsh(SshFileContentSource),
}
