use std::sync::Arc;

#[derive(Clone)]
pub struct HostString(Arc<String>);

impl HostString {
    pub fn new(host: String) -> Self {
        Self(Arc::new(host))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

pub struct HostStr<'s>(&'s str);

impl<'s> HostStr<'s> {
    pub fn new(host: &'s str) -> Self {
        Self(host)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_my_https_server_name(&self, connection_server_name: &str, listen_port: u16) -> bool {
        let (host, port) = parse_https_host_port(self.as_str());

        host == connection_server_name && port == listen_port
    }
}

fn parse_https_host_port(src: &str) -> (&str, u16) {
    let mut parts = src.split(':');
    let host = parts.next().unwrap();
    let port = parts.next().map(|p| p.parse().unwrap()).unwrap_or(443);
    (host, port)
}
