use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    body::Incoming,
    header::{HeaderName, HeaderValue},
    HeaderMap,
};

use crate::settings::ModifyHttpHeadersSettings;

use super::{into_full_bytes, HttpProxyPassInner, LocationIndex, ProxyPassError, SourceHttpData};

pub async fn build_http_response(
    req_uri: &hyper::Uri,
    response: hyper::Response<Incoming>,
    inner: &HttpProxyPassInner,
    location_index: &LocationIndex,
) -> Result<hyper::Response<Full<Bytes>>, ProxyPassError> {
    let (mut parts, incoming) = response.into_parts();

    modify_req_headers(req_uri, inner, &mut parts.headers, location_index);

    let body = into_full_bytes(incoming).await?;
    Ok(hyper::Response::from_parts(parts, body))
}

pub fn build_response_from_content(
    req_uri: &hyper::Uri,
    inner: &HttpProxyPassInner,
    location_index: &LocationIndex,
    content: Vec<u8>,
) -> hyper::Response<Full<Bytes>> {
    let mut builder = hyper::Response::builder();

    if let Some(headers) = builder.headers_mut() {
        modify_req_headers(req_uri, inner, headers, location_index);
    }

    let full_body = http_body_util::Full::new(hyper::body::Bytes::from(content));
    builder.body(full_body).unwrap()
}

fn modify_req_headers(
    req_uri: &hyper::Uri,
    inner: &HttpProxyPassInner,
    headers: &mut HeaderMap<HeaderValue>,
    location_index: &LocationIndex,
) {
    if let Some(modify_headers_settings) = inner
        .modify_headers_settings
        .global_modify_headers_settings
        .as_ref()
    {
        modify_headers(req_uri, headers, modify_headers_settings, &inner.src);
    }

    if let Some(modify_headers_settings) = inner
        .modify_headers_settings
        .endpoint_modify_headers_settings
        .as_ref()
    {
        modify_headers(req_uri, headers, modify_headers_settings, &inner.src);
    }

    let proxy_pass_location = inner.locations.find(location_index);

    if let Some(modify_headers_settings) = proxy_pass_location.modify_headers.as_ref() {
        modify_headers(req_uri, headers, modify_headers_settings, &inner.src);
    }
}

fn modify_headers(
    req_uri: &hyper::Uri,
    headers: &mut HeaderMap<hyper::header::HeaderValue>,
    headers_settings: &ModifyHttpHeadersSettings,
    src: &SourceHttpData,
) {
    if let Some(remove_header) = headers_settings.remove.as_ref() {
        if let Some(remove_headers) = remove_header.response.as_ref() {
            for remove_header in remove_headers {
                headers.remove(remove_header.as_str());
            }
        }
    }

    if let Some(add_headers) = headers_settings.add.as_ref() {
        if let Some(add_headers) = add_headers.response.as_ref() {
            for add_header in add_headers {
                headers.insert(
                    HeaderName::from_bytes(add_header.name.as_bytes()).unwrap(),
                    src.populate_value(&add_header.value, req_uri)
                        .as_str()
                        .parse()
                        .unwrap(),
                );
            }
        }
    }
}
