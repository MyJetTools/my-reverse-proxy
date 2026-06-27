use std::sync::LazyLock;

use rust_extensions::StrOrString;
use x509_parser::nom::AsBytes;

pub static NOT_FOUND: LazyLock<Vec<u8>> =
    LazyLock::new(|| generate_layout(404, "Resource not found", None));

pub static REMOTE_RESOURCE_IS_NOT_AVAILABLE: LazyLock<Vec<u8>> = LazyLock::new(|| {
    generate_layout(503, "Server Error", Some("Remote resource is not available".into()))
});

pub static LOCATION_IS_NOT_FOUND: LazyLock<Vec<u8>> = LazyLock::new(|| {
    generate_layout_with_close(
        503,
        "Server Error",
        Some("Remote location configuration is missing".into()),
    )
});

pub static ENDPOINT_CAN_NOT_BE_UPGRADED_TO_WEB_SOCKET: LazyLock<Vec<u8>> = LazyLock::new(|| {
    generate_layout(
        405,
        "Forbidden",
        Some("Endpoint can not be upgraded to websocket".into()),
    )
});

pub static ERROR_TIMEOUT: LazyLock<Vec<u8>> =
    LazyLock::new(|| generate_layout(503, "Server Error", Some("Timeout".into())));

pub static ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE: LazyLock<Vec<u8>> =
    LazyLock::new(|| generate_layout(502, "Server Error", Some("Bad gateway".into())));

pub static UPSTREAM_IS_NOT_HTTP: LazyLock<Vec<u8>> = LazyLock::new(|| {
    generate_layout(
        502,
        "Server Error",
        Some("Upstream is not a valid HTTP server".into()),
    )
});

pub static NOT_AUTHORIZED_PAGE: LazyLock<Vec<u8>> =
    LazyLock::new(|| generate_layout(401, "Not authorized request", None));

pub static PROXY_TO_HEADER_MISSING: LazyLock<Vec<u8>> = LazyLock::new(|| {
    build_layout_with_explicit_status(
        421,
        "Misdirected Request",
        Some("Missing or invalid proxy-to header".into()),
    )
});

pub static PROXY_TO_HOST_NOT_ALLOWED: LazyLock<Vec<u8>> = LazyLock::new(|| {
    build_layout_with_explicit_status(403, "Forbidden", Some("Upstream host not allowed".into()))
});

pub static MTLS_REQUIRED_MISDIRECTED: LazyLock<Vec<u8>> = LazyLock::new(|| {
    build_layout_with_explicit_status(
        421,
        "Misdirected Request",
        Some("A client certificate is required for this host".into()),
    )
});

pub fn generate_layout(status_code: u16, text: &str, second_line: Option<StrOrString>) -> Vec<u8> {
    build_layout(status_code, text, second_line, false)
}

pub fn generate_layout_with_close(
    status_code: u16,
    text: &str,
    second_line: Option<StrOrString>,
) -> Vec<u8> {
    build_layout(status_code, text, second_line, true)
}

/// Like `build_layout` but writes the actual status code on the response line
/// instead of hardcoding `200 OK`. Used by templates that need to drive client
/// behavior via HTTP status (e.g. 421 Misdirected Request, 403 Forbidden).
fn build_layout_with_explicit_status(
    status_code: u16,
    text: &str,
    second_line: Option<StrOrString>,
) -> Vec<u8> {
    use crate::app::APP_VERSION;

    let second_line = if let Some(second_line) = second_line {
        format!("<h4>{}</h4>", second_line.as_str())
    } else {
        "".to_string()
    };

    let body = format!(
        r#"
        <div style="text-align: center;">
        <h2>{text}</h2>
      {second_line}
        <p>{status_code}</p>
        <hr/>
        <div>MyReverseProxy {APP_VERSION}</div>
        </div>
        "#
    )
    .into_bytes();

    let mut headers = crate::h1_utils::Http1HeadersBuilder::new();
    headers.push_response_first_line(status_code);
    headers.push_content_length(body.len());
    headers.push_cl_cr();

    let mut result = headers.into_bytes();
    result.extend_from_slice(body.as_bytes());
    result
}

fn build_layout(
    status_code: u16,
    text: &str,
    second_line: Option<StrOrString>,
    connection_close: bool,
) -> Vec<u8> {
    use crate::app::APP_VERSION;

    let second_line = if let Some(second_line) = second_line {
        format!("<h4>{}</h4>", second_line.as_str())
    } else {
        "".to_string()
    };

    let body = format!(
        r#"
        <div style="text-align: center;">
        <h2>{text}</h2>
      {second_line}
        <p>{status_code}</p>
        <hr/>
        <div>MyReverseProxy {APP_VERSION}</div>
        </div>
        "#
    )
    .into_bytes();

    let mut headers = crate::h1_utils::Http1HeadersBuilder::new();
    headers.push_response_first_line(status_code);

    headers.push_content_length(body.len());
    if connection_close {
        headers.push_header("Connection", "close");
    }
    headers.push_cl_cr();

    let mut result = headers.into_bytes();
    result.extend_from_slice(body.as_bytes());
    result
}
