use hyper::{header::HeaderValue, HeaderMap, Uri};

pub struct HostPort<'s> {
    uri: &'s Uri,
    host: Option<&'s HeaderValue>,
    headers: &'s HeaderMap<HeaderValue>,
}

impl<'s> HostPort<'s> {
    pub fn new(uri: &'s Uri, headers: &'s HeaderMap<HeaderValue>) -> Self {
        let host = headers.get("host");
        Self { uri, host, headers }
    }

    pub fn is_my_host_port(&self, host_port: &str) -> bool {
        let mut host_port = host_port.split(':');

        let el_0 = host_port.next().unwrap();
        let el_1 = host_port.next();

        let (host, port) = if let Some(el_1) = el_1 {
            (Some(el_0), el_1)
        } else {
            (None, el_0)
        };

        let port = port.parse::<u16>();

        if port.is_err() {
            return false;
        }

        let port = port.unwrap();

        if let Some(host) = host {
            return self.get_host() == Some(host) && self.get_port() == port;
        }

        self.get_port() == port
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

    pub fn is_https(&self) -> bool {
        if let Some(scheme_str) = self.uri.scheme_str() {
            return rust_extensions::str_utils::compare_strings_case_insensitive(
                scheme_str, "https",
            );
        }

        println!(
            "HostPort::is_https. No scheme found in uri: {:?}. Headers: {:#?}",
            self.uri, self.headers
        );

        return false;
    }

    pub fn get_port_opt(&self) -> Option<u16> {
        self.uri.port_u16()
    }

    pub fn get_port(&self) -> u16 {
        if let Some(port) = self.uri.port_u16() {
            return port;
        }

        if let Some(host) = self.host {
            if let Ok(host) = host.to_str() {
                let port = host.split(':').last();
                if let Some(port) = port {
                    if let Ok(port) = port.parse::<u16>() {
                        return port;
                    }
                }
            }
        }

        if self.is_https() {
            443
        } else {
            80
        }
    }
}
