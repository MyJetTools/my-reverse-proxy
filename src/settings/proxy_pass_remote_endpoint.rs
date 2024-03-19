use hyper::Uri;

use super::SshConfiguration;

pub enum ProxyPassRemoteEndpoint {
    Http(Uri),
    Http2(Uri),
    Http1OverSsh(SshConfiguration),
    Http2OverSsh(SshConfiguration),
}
