use http::Method;

use super::MyHttpRequest;
use crate::headers::{validate_header_name, validate_header_value};

pub struct MyHttpRequestBuilder {
    headers: Vec<u8>,
}

impl MyHttpRequestBuilder {
    pub fn new(method: Method, path_and_query: &str) -> Self {
        for &b in path_and_query.as_bytes() {
            if b == b'\r' || b == b'\n' || b == 0 || b == b' ' {
                panic!(
                    "Request path contains forbidden byte 0x{:02x} (request line injection)",
                    b
                );
            }
        }
        let mut headers = Vec::new();
        headers.extend_from_slice(method.as_str().as_bytes());
        headers.push(b' ');
        headers.extend_from_slice(path_and_query.as_bytes());
        headers.push(b' ');
        headers.extend_from_slice(b"HTTP/1.1\r\n");
        Self { headers }
    }

    pub fn append_header(&mut self, name: &str, value: &str) {
        validate_header_name(name);
        validate_header_value(value);
        self.headers.extend_from_slice(name.as_bytes());
        self.headers.push(b':');
        self.headers.push(b' ');
        self.headers.extend_from_slice(value.as_bytes());
        self.headers.extend_from_slice(crate::CL_CR);
    }

    pub fn build_with_body(mut self, body: Vec<u8>) -> MyHttpRequest {
        if !body.is_empty() && !super::headers_contains(&self.headers, "content-length") {
            self.append_header("Content-Length", body.len().to_string().as_str());
        }

        MyHttpRequest {
            headers: self.headers,
            body: body.into(),
        }
    }

    pub fn build(self) -> MyHttpRequest {
        MyHttpRequest {
            headers: self.headers,
            body: Vec::new().into(),
        }
    }
}
