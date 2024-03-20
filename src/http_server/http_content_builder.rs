use std::collections::HashMap;

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    header::{HeaderName, HeaderValue},
    HeaderMap, Uri,
};

use super::{HostPort, ProxyPassError, ProxyPassInner, SourceHttpConfiguration};

pub struct HttpContentBuilder {
    src: Option<hyper::Request<hyper::body::Incoming>>,
    result: Option<hyper::Request<Full<Bytes>>>,
}

impl HttpContentBuilder {
    pub fn new(src: hyper::Request<hyper::body::Incoming>) -> Self {
        Self {
            src: Some(src),
            result: None,
        }
    }

    pub async fn populate_and_build(
        &mut self,
        inner: &ProxyPassInner,
    ) -> Result<(), ProxyPassError> {
        if self.result.is_some() {
            return Ok(());
        }

        let (mut parts, incoming) = self.src.take().unwrap().into_parts();

        if let Some(fill_headers) = inner.populate_request_headers.as_ref() {
            populate_request_headers(&mut parts.headers, fill_headers, &inner.src);
        }

        let body = into_full_bytes(incoming).await?;

        self.result = Some(hyper::Request::from_parts(parts, body));

        Ok(())
    }

    pub fn uri(&self) -> &Uri {
        if let Some(src) = self.src.as_ref() {
            return src.uri();
        }

        self.result.as_ref().unwrap().uri()
    }

    pub fn get_host_port<'s>(&'s self) -> HostPort<'s> {
        if let Some(src) = self.src.as_ref() {
            return HostPort::new(src.uri(), src.headers());
        }

        let result = self.result.as_ref().unwrap();
        return HostPort::new(result.uri(), result.headers());
    }

    pub fn get(&self) -> hyper::Request<Full<Bytes>> {
        self.result.as_ref().unwrap().clone()
    }
}

pub async fn into_full_bytes(
    incoming: impl hyper::body::Body<Data = hyper::body::Bytes, Error = hyper::Error>,
) -> Result<Full<Bytes>, ProxyPassError> {
    use http_body_util::BodyExt;

    let collected = incoming.collect().await?;
    let bytes = collected.to_bytes();

    let body = http_body_util::Full::new(bytes);
    Ok(body)
}

fn populate_request_headers(
    headers: &mut HeaderMap<HeaderValue>,
    fill_headers: &HashMap<String, String>,
    src: &SourceHttpConfiguration,
) {
    for (header_name, header_value) in fill_headers {
        headers.insert(
            HeaderName::from_bytes(header_name.as_bytes()).unwrap(),
            src.populate_value(header_value).as_str().parse().unwrap(),
        );
    }
}
