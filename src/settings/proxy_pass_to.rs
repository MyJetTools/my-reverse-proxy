use std::str::FromStr;

use hyper::Uri;

use super::{HttpProxyPassRemoteEndpoint, SshConfiguration};

pub struct ProxyPassTo(String);

impl ProxyPassTo {
    pub fn new(location: String) -> Self {
        Self(location)
    }

    pub fn is_ssh(&self) -> bool {
        self.0.starts_with("ssh")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_ssh_configuration(&self) -> Option<SshConfiguration> {
        if self.is_ssh() {
            return Some(SshConfiguration::parse(self.as_str()));
        }

        None
    }

    pub fn to_http_remote_endpoint(&self, is_http1: bool) -> HttpProxyPassRemoteEndpoint {
        if let Some(ssh_configuration) = self.to_ssh_configuration() {
            if is_http1 {
                HttpProxyPassRemoteEndpoint::Http1OverSsh(ssh_configuration)
            } else {
                HttpProxyPassRemoteEndpoint::Http2OverSsh(ssh_configuration)
            }
        } else {
            if is_http1 {
                HttpProxyPassRemoteEndpoint::Http(Uri::from_str(self.as_str()).unwrap())
            } else {
                HttpProxyPassRemoteEndpoint::Http2(Uri::from_str(self.as_str()).unwrap())
            }
        }
    }
}
