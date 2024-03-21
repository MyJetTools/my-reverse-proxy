use std::sync::Arc;

use hyper::Uri;
use my_ssh::{SshCredentials, SshRemoteHost};

use super::FileName;

pub enum ContentSourceSettings<'s> {
    Http(HttpProxyPassRemoteEndpoint),
    File {
        file_name: FileName<'s>,
        default_file: Option<String>,
    },
    FileOverSsh {
        ssh_credentials: Arc<SshCredentials>,
        file_path: String,
        default_file: Option<String>,
    },
}

pub enum HttpProxyPassRemoteEndpoint {
    Http(Uri),
    Http2(Uri),
    Http1OverSsh {
        ssh_credentials: Arc<SshCredentials>,
        remote_host: SshRemoteHost,
    },
    Http2OverSsh {
        ssh_credentials: Arc<SshCredentials>,
        remote_host: SshRemoteHost,
    },
}

impl HttpProxyPassRemoteEndpoint {
    pub fn is_http1(&self) -> bool {
        match self {
            HttpProxyPassRemoteEndpoint::Http(_) => true,
            HttpProxyPassRemoteEndpoint::Http2(_) => false,
            HttpProxyPassRemoteEndpoint::Http1OverSsh {
                ssh_credentials: _,
                remote_host: _,
            } => true,
            HttpProxyPassRemoteEndpoint::Http2OverSsh {
                ssh_credentials: _,
                remote_host: _,
            } => false,
        }
    }
}
