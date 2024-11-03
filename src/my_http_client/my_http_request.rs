use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use std::fmt::Write;

use crate::http_proxy_pass::HostPort;

pub struct MyHttpRequest {
    headers: Vec<u8>,
    body: Bytes,
}

impl MyHttpRequest {
    pub async fn new(req: hyper::Request<Full<Bytes>>) -> Self {
        let mut headers = String::new();

        write!(
            &mut headers,
            "{} {} {:?}\r\n",
            req.method(),
            req.uri()
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/"),
            req.version()
        )
        .unwrap();

        let (parts, body) = req.into_parts();

        for (name, value) in parts.get_headers() {
            write!(&mut headers, "{}: {}\r\n", name, value.to_str().unwrap()).unwrap();
        }

        // End headers section
        headers.push_str("\r\n");

        let body_as_bytes = body.collect().await.unwrap().to_bytes();

        Self {
            headers: headers.into_bytes(),
            body: body_as_bytes,
        }
    }

    pub fn write_to(&self, writer: &mut Vec<u8>) {
        writer.extend_from_slice(&self.headers);
        writer.extend_from_slice(&self.body);
    }
}
