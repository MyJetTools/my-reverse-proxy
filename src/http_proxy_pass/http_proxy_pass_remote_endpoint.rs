use std::sync::Arc;

use my_ssh::SshCredentials;

use crate::configurations::*;

#[derive(Debug)]
pub enum HttpProxyPassRemoteEndpoint {
    Http(RemoteHost),
    Http2(RemoteHost),
    Http1OverSsh {
        ssh_credentials: Arc<SshCredentials>,
        remote_host: RemoteHost,
    },
    Http2OverSsh {
        ssh_credentials: Arc<SshCredentials>,
        remote_host: RemoteHost,
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
