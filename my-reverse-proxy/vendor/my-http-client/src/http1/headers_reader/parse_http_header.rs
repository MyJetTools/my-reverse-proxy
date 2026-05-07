use http::HeaderValue;
use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use crate::http1::{DetectedBodySize, HttpParseError};

pub fn parse_http_header(
    mut builder: http::response::Builder,
    src: &[u8],
) -> Result<(http::response::Builder, DetectedBodySize), HttpParseError> {
    let mut body_size = DetectedBodySize::Unknown;
    let pos = src.find_byte_pos(b':', 0);

    if pos.is_none() {
        return Err(HttpParseError::invalid_payload(
            "Can not find separator between HTTP header and Http response",
        ));
    }

    let pos = pos.unwrap();

    let name = &src[..pos];
    let name = std::str::from_utf8(name).map_err(|_| {
        HttpParseError::invalid_payload(
            "Invalid HTTP header name. Can not convert payload to UTF8 string",
        )
    })?;

    let value = &src[pos + 1..];
    let value_str = std::str::from_utf8(value).map_err(|_| {
        HttpParseError::invalid_payload(
            "Invalid HTTP value. Can not convert payload to UTF8 string",
        )
    })?;

    let value_str = value_str.trim();

    if name.eq_ignore_ascii_case("Content-Length") {
        match value_str.parse() {
            Ok(value) => body_size = DetectedBodySize::Known(value),
            Err(_) => {
                let value_str = if value_str.len() > 16 {
                    &value_str[..16]
                } else {
                    value_str
                };
                return Err(HttpParseError::invalid_payload(format!(
                    "Invalid Content-Length value: {}",
                    value_str
                )));
            }
        }
    }

    if name.eq_ignore_ascii_case("Transfer-Encoding")
        && value_str.eq_ignore_ascii_case("chunked")
    {
        body_size = DetectedBodySize::Chunked;
    }

    if name.eq_ignore_ascii_case("upgrade") && value_str.eq_ignore_ascii_case("websocket") {
        body_size = DetectedBodySize::WebSocketUpgrade;
    }

    let header_value = HeaderValue::from_str(value_str).map_err(|err| {
        HttpParseError::invalid_payload(format!(
            "Invalid Header value. {}: {}. Err: {}",
            name, value_str, err
        ))
    })?;

    builder = builder.header(name, header_value);

    Ok((builder, body_size))
}
