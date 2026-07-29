use hyper::{header::HeaderValue, HeaderMap, Uri};
use hyper_tungstenite::tungstenite::http::request::Parts;
use rust_extensions::ShortString;

pub trait HostPort {
    fn get_uri(&self) -> &Uri;
    fn get_headers(&self) -> &HeaderMap<HeaderValue>;

    fn get_host(&self) -> Option<&str> {
        if let Some(host_value) = self.get_headers().get("host") {
            if let Ok(value) = host_value.to_str() {
                return Some(value.into());
            }
        }

        if let Some(host) = self.get_uri().host() {
            return Some(host);
        }

        None
    }

    fn get_host_port(&self) -> ShortString {
        let mut result = ShortString::new_empty();

        if let Some(host) = self.get_host() {
            result.push_str(host);
        }

        if let Some(port) = self.get_port() {
            result.push_str(":");
            result.push_str(port.to_string().as_str());
        }

        result
    }

    fn get_port(&self) -> Option<u16> {
        self.get_uri().port_u16()
    }

    fn get_path_and_query(&self) -> Option<&str> {
        self.get_uri().path_and_query()?.as_str().into()
    }
}

impl<T> HostPort for hyper::Request<T> {
    fn get_uri(&self) -> &Uri {
        self.uri()
    }

    fn get_headers(&self) -> &HeaderMap<HeaderValue> {
        self.headers()
    }
}

impl HostPort for Parts {
    fn get_uri(&self) -> &Uri {
        &self.uri
    }

    fn get_headers(&self) -> &HeaderMap<HeaderValue> {
        &self.headers
    }
}
