use hyper::Uri;

//todo - unit test it
#[derive(Debug, Clone)]
pub struct RemoteHost(String);

impl RemoteHost {
    pub fn new(remote_host: String) -> Self {
        Self(remote_host)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }

    pub fn to_uri(&self) -> Uri {
        self.0.parse().unwrap()
    }

    pub fn get_host_port(&self) -> &str {
        if let Some(index) = self.0.find("://") {
            return &self.0[index + 3..];
        } else {
            return self.0.as_str();
        }
    }

    pub fn is_https(&self) -> bool {
        self.0.starts_with("https")
    }

    pub fn get_host(&self) -> &str {
        let host_port = self.get_host_port();

        if let Some(port_separator) = host_port.find(":") {
            return &host_port[..port_separator];
        } else {
            return host_port;
        }
    }

    pub fn get_port(&self) -> u16 {
        let host_port = self.get_host_port();

        if let Some(port_separator) = host_port.find(":") {
            return host_port[port_separator + 1..].parse().unwrap();
        }

        if self.is_https() {
            return 443;
        } else {
            80
        }
    }

    pub fn is_http(&self) -> bool {
        self.0.starts_with("http")
    }
}

impl Into<RemoteHost> for String {
    fn into(self) -> RemoteHost {
        RemoteHost::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::RemoteHost;

    #[test]
    fn test_tcp_remote_host() {
        let remote_host = RemoteHost::new("192.168.1.5:8080".to_string());
        assert_eq!(remote_host.get_host(), "192.168.1.5");
        assert_eq!(remote_host.get_port(), 8080);
    }

    #[test]
    fn test_default_port() {
        let remote_host = RemoteHost::new("192.168.1.5".to_string());
        assert_eq!(remote_host.get_host(), "192.168.1.5");
        assert_eq!(remote_host.get_host_port(), "192.168.1.5");
        assert_eq!(remote_host.get_port(), 80);
    }

    #[test]
    fn test_https_host() {
        let remote_host = RemoteHost::new("https://domain:443".to_string());
        assert_eq!(remote_host.get_host(), "domain");
        assert_eq!(remote_host.get_host_port(), "domain:443");
        assert_eq!(remote_host.get_port(), 443);
        assert_eq!(remote_host.is_https(), true);
    }

    #[test]
    fn test_http_host() {
        let remote_host = RemoteHost::new("http://domain:8080".to_string());
        assert_eq!(remote_host.get_host(), "domain");
        assert_eq!(remote_host.get_host_port(), "domain:8080");
        assert_eq!(remote_host.get_port(), 8080);
        assert_eq!(remote_host.is_https(), false);
    }

    #[test]
    fn test_default_port_on_http() {
        let remote_host = RemoteHost::new("http://domain".to_string());
        assert_eq!(remote_host.get_host(), "domain");
        assert_eq!(remote_host.get_host_port(), "domain");
        assert_eq!(remote_host.get_port(), 80);
        assert_eq!(remote_host.is_https(), false);
    }
}
