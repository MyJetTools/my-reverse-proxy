use hyper::{
    header::{self, HeaderName, HeaderValue},
    HeaderMap,
};

use crate::{configurations::ModifyHeadersConfig, types::HttpRequestReader};

use super::{HttpProxyPass, HttpProxyPassInner, LocationIndex};

pub fn modify_resp_headers(
    proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    req_host_port: &impl HttpRequestReader,
    headers: &mut HeaderMap<HeaderValue>,
    location_index: &LocationIndex,
) {
    let proxy_pass_location = inner.locations.find(location_index);

    if let Some(dest_http1) = proxy_pass_location.is_http1() {
        if dest_http1 && !proxy_pass.listening_port_info.endpoint_type.is_http1() {
            headers.remove(header::TRANSFER_ENCODING);
            headers.remove(header::CONNECTION);
            headers.remove(header::UPGRADE);
            headers.remove("keep-alive");
            headers.remove("proxy-connection");
            headers.remove("connection");
        }
    }

    modify_headers(
        inner,
        req_host_port,
        headers,
        &proxy_pass.endpoint_info.modify_response_headers,
    );

    modify_headers(
        inner,
        req_host_port,
        headers,
        &proxy_pass_location.config.modify_response_headers,
    );
}

fn modify_headers(
    inner: &HttpProxyPassInner,
    req_host_port: &impl HttpRequestReader,
    headers: &mut HeaderMap<hyper::header::HeaderValue>,
    headers_settings: &ModifyHeadersConfig,
) {
    for remove in headers_settings.iter_remove() {
        headers.remove(remove);
    }

    for (header, value) in headers_settings.iter_add() {
        headers.insert(
            HeaderName::from_bytes(header.as_bytes()).unwrap(),
            inner
                .populate_value(&value, req_host_port)
                .as_str()
                .parse()
                .unwrap(),
        );
    }
}
