use http::HeaderValue;
use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use crate::my_http_client::{DetectedBodySize, HttpParseError};

pub fn parse_http_header(
    mut builder: http::response::Builder,
    src: &[u8],
) -> Result<(http::response::Builder, DetectedBodySize), HttpParseError> {
    let mut body_size = DetectedBodySize::Unknown;
    let pos = src.find_byte_pos(b':', 0);

    if pos.is_none() {
        return Err(HttpParseError::Error(format!(
            "Invalid header {}",
            std::str::from_utf8(src).unwrap()
        )));
    }

    let pos = pos.unwrap();

    let name = &src[..pos];
    let name = std::str::from_utf8(name).unwrap();

    let value = &src[pos + 1..];
    let value_str = std::str::from_utf8(value).unwrap().trim();

    if name.eq_ignore_ascii_case("Content-Length") {
        match value_str.parse() {
            Ok(value) => body_size = DetectedBodySize::Known(value),
            Err(_) => {
                return Err(HttpParseError::Error(format!(
                    "Invalid Content-Length value: {}",
                    value_str
                )));
            }
        }
    }

    if name.eq_ignore_ascii_case("Transfer-Encoding") {
        if value_str.eq_ignore_ascii_case("chunked") {
            body_size = DetectedBodySize::Chunked;
        }
    }

    if name.eq_ignore_ascii_case("upgrade") {
        if value_str.eq_ignore_ascii_case("websocket") {
            body_size = DetectedBodySize::WebSocketUpgrade;
        }
    }

    builder = builder.header(name, HeaderValue::from_bytes(value).unwrap());

    // builder.header(name, value);

    Ok((builder, body_size))
}
