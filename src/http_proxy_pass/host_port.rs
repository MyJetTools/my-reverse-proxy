use hyper::{header::HeaderValue, HeaderMap, Uri};

pub struct HostPort<'s> {
    uri: &'s Uri,
    host: Option<&'s HeaderValue>,
}

impl<'s> HostPort<'s> {
    pub fn new(uri: &'s Uri, headers: &'s HeaderMap<HeaderValue>) -> Self {
        let host = headers.get("host");
        Self { uri, host }
    }

    pub fn get_host(&self) -> Option<&'s str> {
        if let Some(host_value) = self.host {
            if let Ok(value) = host_value.to_str() {
                return Some(value.into());
            }
        }

        if let Some(host) = self.uri.host() {
            return Some(host);
        }

        None
    }

    pub fn get_path_and_query(&self) -> Option<&str> {
        self.uri.path_and_query()?.as_str().into()
    }

    pub fn get_port_opt(&self) -> Option<u16> {
        self.uri.port_u16()
    }
}
