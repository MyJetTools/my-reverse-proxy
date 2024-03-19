pub struct HostPort<'s, T> {
    request: &'s hyper::Request<T>,
}

impl<'s, T> HostPort<'s, T> {
    pub fn new(request: &'s hyper::Request<T>) -> Self {
        Self { request }
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
        if let Some(host_value) = self.request.headers().get("host") {
            if let Ok(value) = host_value.to_str() {
                return Some(value.into());
            }
        }

        if let Some(host) = self.request.uri().host() {
            return Some(host);
        }

        None
    }

    pub fn is_http(&self) -> bool {
        self.request
            .uri()
            .scheme_str()
            .unwrap_or_default()
            .to_lowercase()
            == "http"
    }

    pub fn get_port(&self) -> u16 {
        if let Some(port) = self.request.uri().port() {
            port.as_u16()
        } else {
            if self.is_http() {
                80
            } else {
                443
            }
        }
    }
}
