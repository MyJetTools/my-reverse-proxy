use bytes::Bytes;
use http::{Method, Version};
use http_body_util::{BodyExt, Full};
use std::fmt::Write;

#[derive(Clone)]
pub struct MyHttpRequest {
    pub headers: Vec<u8>,
    pub body: Bytes,
}

impl MyHttpRequest {
    pub fn new<Headers: crate::MyHttpClientHeaders>(
        method: Method,
        path_and_query: &str,
        version: Version,
        headers_src: &Headers,
        body: Vec<u8>,
    ) -> Self {
        let mut result = Self {
            headers: create_headers(method, path_and_query, version).into_bytes(),
            body: body.into(),
        };

        headers_src.copy_to(&mut result.headers);

        if !result.body.is_empty()
            && !super::headers_contains(&result.headers, super::CONTENT_LENGTH_HEADER_NAME)
        {
            crate::headers::write_header(
                &mut result.headers,
                super::CONTENT_LENGTH_HEADER_NAME,
                result.body.len().to_string().as_str(),
            );
        }

        result
    }

    pub async fn from_hyper_request(req: hyper::Request<Full<Bytes>>) -> Self {
        let (parts, body) = req.into_parts();

        let headers = create_headers(
            parts.method,
            parts
                .uri
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/"),
            parts.version,
        );

        let mut headers = headers.into_bytes();

        for header in parts.headers.iter() {
            crate::headers::write_header(
                &mut headers,
                header.0.as_str(),
                header.1.to_str().unwrap(),
            );
        }

        let body_as_bytes = body.collect().await.unwrap().to_bytes();

        Self {
            headers,
            body: body_as_bytes,
        }
    }

    pub fn write_to(&self, writer: &mut Vec<u8>) {
        writer.extend_from_slice(&self.headers);
        writer.extend_from_slice(crate::CL_CR);
        writer.extend_from_slice(&self.body);
    }
}

fn create_headers(method: Method, path_and_query: &str, version: Version) -> String {
    let mut headers = String::new();

    write!(
        &mut headers,
        "{} {} {:?}\r\n",
        method, path_and_query, version
    )
    .unwrap();

    headers
}
