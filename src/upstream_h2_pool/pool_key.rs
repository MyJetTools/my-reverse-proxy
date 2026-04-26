use rust_extensions::remote_endpoint::RemoteEndpointOwned;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum H2Scheme {
    Http2,
    Https2,
    UnixHttp2,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PoolKey {
    pub scheme: H2Scheme,
    pub host: String,
    pub port: u16,
}

impl PoolKey {
    pub fn new_tcp(scheme: H2Scheme, host: &str, port: u16) -> Self {
        Self {
            scheme,
            host: host.to_ascii_lowercase(),
            port,
        }
    }

    pub fn new_uds(socket_path: &str) -> Self {
        Self {
            scheme: H2Scheme::UnixHttp2,
            host: socket_path.to_string(),
            port: 0,
        }
    }

    pub fn from_remote_endpoint(scheme: H2Scheme, ep: &RemoteEndpointOwned) -> Self {
        match scheme {
            H2Scheme::UnixHttp2 => Self::new_uds(ep.get_host_port().as_str()),
            H2Scheme::Http2 => Self::new_tcp(scheme, ep.get_host(), ep.get_port().unwrap_or(80)),
            H2Scheme::Https2 => Self::new_tcp(scheme, ep.get_host(), ep.get_port().unwrap_or(443)),
        }
    }

    pub fn endpoint_label(&self) -> String {
        match self.scheme {
            H2Scheme::Http2 => format!("h2://{}:{}", self.host, self.port),
            H2Scheme::Https2 => format!("h2s://{}:{}", self.host, self.port),
            H2Scheme::UnixHttp2 => format!("uds-h2://{}", self.host),
        }
    }
}
