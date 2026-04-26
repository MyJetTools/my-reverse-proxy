use rust_extensions::remote_endpoint::RemoteEndpointOwned;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum H1Scheme {
    Http1,
    Https1,
    UnixHttp1,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PoolKey {
    pub scheme: H1Scheme,
    pub host: String,
    pub port: u16,
}

impl PoolKey {
    pub fn new_tcp(scheme: H1Scheme, host: &str, port: u16) -> Self {
        Self {
            scheme,
            host: host.to_ascii_lowercase(),
            port,
        }
    }

    pub fn new_uds(socket_path: &str) -> Self {
        Self {
            scheme: H1Scheme::UnixHttp1,
            host: socket_path.to_string(),
            port: 0,
        }
    }

    pub fn from_remote_endpoint(scheme: H1Scheme, ep: &RemoteEndpointOwned) -> Self {
        match scheme {
            H1Scheme::UnixHttp1 => Self::new_uds(ep.get_host_port().as_str()),
            H1Scheme::Http1 => Self::new_tcp(scheme, ep.get_host(), ep.get_port().unwrap_or(80)),
            H1Scheme::Https1 => Self::new_tcp(scheme, ep.get_host(), ep.get_port().unwrap_or(443)),
        }
    }

    pub fn endpoint_label(&self) -> String {
        match self.scheme {
            H1Scheme::Http1 => format!("h1://{}:{}", self.host, self.port),
            H1Scheme::Https1 => format!("h1s://{}:{}", self.host, self.port),
            H1Scheme::UnixHttp1 => format!("uds-h1://{}", self.host),
        }
    }
}
