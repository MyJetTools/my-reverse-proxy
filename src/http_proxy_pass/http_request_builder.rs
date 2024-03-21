use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    header::{HeaderName, HeaderValue},
    HeaderMap, Uri,
};

use crate::settings::ModifyHttpHeadersSettings;

use super::{HostPort, HttpProxyPassInner, LocationIndex, ProxyPassError, SourceHttpConfiguration};

pub struct HttpRequestBuilder {
    src: Option<hyper::Request<hyper::body::Incoming>>,
    result: Option<hyper::Request<Full<Bytes>>>,
}

impl HttpRequestBuilder {
    pub fn new(src: hyper::Request<hyper::body::Incoming>) -> Self {
        Self {
            src: Some(src),
            result: None,
        }
    }

    pub async fn populate_and_build(
        &mut self,
        inner: &HttpProxyPassInner,
    ) -> Result<LocationIndex, ProxyPassError> {
        let location_index = inner.locations.find_location_index(self.uri())?;
        if self.result.is_some() {
            return Ok(location_index);
        }

        let (mut parts, incoming) = self.src.take().unwrap().into_parts();

        if let Some(modify_headers_settings) = inner
            .modify_headers_settings
            .global_modify_headers_settings
            .as_ref()
        {
            modify_headers(&mut parts.headers, modify_headers_settings, &inner.src);
        }

        if let Some(modify_headers_settings) = inner
            .modify_headers_settings
            .endpoint_modify_headers_settings
            .as_ref()
        {
            modify_headers(&mut parts.headers, modify_headers_settings, &inner.src);
        }

        let proxy_pass_location = inner.locations.find(&location_index);

        if let Some(modify_headers_settings) = proxy_pass_location.modify_headers.as_ref() {
            modify_headers(&mut parts.headers, modify_headers_settings, &inner.src);
        }

        let body = into_full_bytes(incoming).await?;

        self.result = Some(hyper::Request::from_parts(parts, body));

        Ok(location_index)
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

fn modify_headers(
    headers: &mut HeaderMap<HeaderValue>,
    headers_settings: &ModifyHttpHeadersSettings,
    src: &SourceHttpConfiguration,
) {
    if let Some(remove_header) = headers_settings.remove.as_ref() {
        if let Some(remove_headers) = remove_header.request.as_ref() {
            for remove_header in remove_headers {
                headers.remove(remove_header.as_str());
            }
        }
    }

    if let Some(add_headers) = headers_settings.add.as_ref() {
        if let Some(add_headers) = add_headers.request.as_ref() {
            for add_header in add_headers {
                headers.insert(
                    HeaderName::from_bytes(add_header.name.as_bytes()).unwrap(),
                    src.populate_value(&add_header.value)
                        .as_str()
                        .parse()
                        .unwrap(),
                );
            }
        }
    }
}
