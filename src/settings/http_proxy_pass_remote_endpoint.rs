use hyper::Uri;

use super::SshConfiguration;

pub enum HttpProxyPassRemoteEndpoint {
    Http(Uri),
    Http2(Uri),
    Http1OverSsh(SshConfiguration),
    Http2OverSsh(SshConfiguration),
}

impl HttpProxyPassRemoteEndpoint {
    pub fn is_http1(&self) -> bool {
        match self {
            HttpProxyPassRemoteEndpoint::Http(_) => true,
            HttpProxyPassRemoteEndpoint::Http2(_) => false,
            HttpProxyPassRemoteEndpoint::Http1OverSsh(_) => true,
            HttpProxyPassRemoteEndpoint::Http2OverSsh(_) => false,
        }
    }
}
