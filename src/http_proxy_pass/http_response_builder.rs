use hyper::{
    header::{self, HeaderName, HeaderValue},
    HeaderMap,
};

use crate::settings::ModifyHttpHeadersSettings;

use super::{HostPort, HttpProxyPass, HttpProxyPassInner, LocationIndex};

pub fn modify_resp_headers<THostPort: HostPort + Send + Sync + 'static>(
    proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
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

    if let Some(modify_headers_settings) = proxy_pass
        .endpoint_info
        .modify_headers_settings
        .global_modify_headers_settings
        .as_ref()
    {
        modify_headers(inner, req_host_port, headers, modify_headers_settings);
    }

    if let Some(modify_headers_settings) = proxy_pass
        .endpoint_info
        .modify_headers_settings
        .endpoint_modify_headers_settings
        .as_ref()
    {
        modify_headers(inner, req_host_port, headers, modify_headers_settings);
    }

    if let Some(modify_headers_settings) = proxy_pass_location.config.modify_headers.as_ref() {
        modify_headers(inner, req_host_port, headers, modify_headers_settings);
    }
}

fn modify_headers<THostPort: HostPort + Send + Sync + 'static>(
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
    headers: &mut HeaderMap<hyper::header::HeaderValue>,
    headers_settings: &ModifyHttpHeadersSettings,
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
                    inner
                        .populate_value(&add_header.value, req_host_port)
                        .as_str()
                        .parse()
                        .unwrap(),
                );
            }
        }
    }
}
