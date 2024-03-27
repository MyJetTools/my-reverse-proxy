use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    body::Incoming,
    header::{self, HeaderName, HeaderValue},
    HeaderMap,
};

use crate::{http_content_source::WebContentType, settings::ModifyHttpHeadersSettings};

use super::{
    into_full_bytes, HostPort, HttpProxyPassInner, HttpType, LocationIndex, ProxyPassError,
    SourceHttpData,
};

pub async fn build_http_response<THostPort: HostPort + Send + Sync + 'static>(
    req_host_port: &THostPort,
    response: hyper::Response<Incoming>,
    inner: &HttpProxyPassInner,
    location_index: &LocationIndex,
    src: HttpType,
    dest_http1: bool,
    x_auth_user: Option<&str>,
) -> Result<hyper::Response<Full<Bytes>>, ProxyPassError> {
    let (mut parts, incoming) = response.into_parts();

    if dest_http1 && !src.is_http1() {
        parts.headers.remove(header::TRANSFER_ENCODING);
        parts.headers.remove(header::CONNECTION);
        parts.headers.remove(header::UPGRADE);
        parts.headers.remove("keep-alive");
        parts.headers.remove("proxy-connection");
        parts.headers.remove("connection");
    }

    modify_req_headers(
        req_host_port,
        inner,
        &mut parts.headers,
        location_index,
        x_auth_user,
    );

    let body = into_full_bytes(incoming).await?;
    Ok(hyper::Response::from_parts(parts, body))
}

pub fn build_response_from_content<THostPort: HostPort + Send + Sync + 'static>(
    req_host_port: &THostPort,
    inner: &HttpProxyPassInner,
    location_index: &LocationIndex,
    content_type: Option<WebContentType>,
    status_code: u16,
    content: Vec<u8>,
    x_auth_user: Option<&str>,
) -> hyper::Response<Full<Bytes>> {
    let mut builder = hyper::Response::builder().status(status_code);

    if let Some(content_type) = content_type {
        builder = builder.header("Content-Type", content_type.as_str());
    }

    if let Some(headers) = builder.headers_mut() {
        modify_req_headers(req_host_port, inner, headers, location_index, x_auth_user);
    }

    let full_body = http_body_util::Full::new(hyper::body::Bytes::from(content));
    builder.body(full_body).unwrap()
}

fn modify_req_headers<THostPort: HostPort + Send + Sync + 'static>(
    req_host_port: &THostPort,
    inner: &HttpProxyPassInner,
    headers: &mut HeaderMap<HeaderValue>,
    location_index: &LocationIndex,
    x_auth_user: Option<&str>,
) {
    if let Some(modify_headers_settings) = inner
        .modify_headers_settings
        .global_modify_headers_settings
        .as_ref()
    {
        modify_headers(
            req_host_port,
            headers,
            modify_headers_settings,
            &inner.src,
            x_auth_user,
        );
    }

    if let Some(modify_headers_settings) = inner
        .modify_headers_settings
        .endpoint_modify_headers_settings
        .as_ref()
    {
        modify_headers(
            req_host_port,
            headers,
            modify_headers_settings,
            &inner.src,
            x_auth_user,
        );
    }

    let proxy_pass_location = inner.locations.find(location_index);

    if let Some(modify_headers_settings) = proxy_pass_location.modify_headers.as_ref() {
        modify_headers(
            req_host_port,
            headers,
            modify_headers_settings,
            &inner.src,
            x_auth_user,
        );
    }
}

fn modify_headers<THostPort: HostPort + Send + Sync + 'static>(
    req_host_port: &THostPort,
    headers: &mut HeaderMap<hyper::header::HeaderValue>,
    headers_settings: &ModifyHttpHeadersSettings,
    src: &SourceHttpData,
    x_auth_user: Option<&str>,
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
                    src.populate_value(&add_header.value, req_host_port, x_auth_user)
                        .as_str()
                        .parse()
                        .unwrap(),
                );
            }
        }
    }
}
