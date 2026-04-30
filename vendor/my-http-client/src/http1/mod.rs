mod my_http_client;
use std::time::Duration;

pub use my_http_client::*;
mod detected_body_size;
pub use detected_body_size::*;
mod my_http_client_inner;
pub use my_http_client_inner::*;
mod queue_of_requests;
mod read_loop;
mod write_loop;
pub use queue_of_requests::*;
mod my_http_response;
pub use my_http_response::*;

mod my_http_request;
pub use my_http_request::*;
mod my_http_request_builder;

mod my_http_client_connection_context;
pub use my_http_client_connection_context::*;

pub use my_http_request_builder::*;

mod tcp_buffer;
use rust_extensions::StrOrString;
pub use tcp_buffer::*;

mod body_reader;
pub use body_reader::*;
mod headers_reader;
pub use headers_reader::*;

mod my_http_client_metrics;
pub use my_http_client_metrics::*;

mod read_with_timeout;
pub use read_with_timeout::*;

pub mod into_hyper_request;

const CONTENT_LENGTH_HEADER_NAME: &str = "content-length";

pub const MAX_RESPONSE_BODY_SIZE: usize = 100 * 1024 * 1024;
pub const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024;
pub const MAX_RESPONSE_HEADERS_COUNT: usize = 256;

/// Case-insensitive search for an HTTP header in a serialized request buffer.
/// Skips the request line; matches lines starting with `\r\n<name>` followed by `:` (or OWS+`:`).
pub(crate) fn headers_contains(buf: &[u8], name: &str) -> bool {
    let needle = name.as_bytes();
    if needle.is_empty() {
        return false;
    }
    let mut i = 0;
    while i + 2 + needle.len() < buf.len() {
        if buf[i] == b'\r' && buf[i + 1] == b'\n' {
            let candidate_start = i + 2;
            let candidate_end = candidate_start + needle.len();
            if candidate_end <= buf.len()
                && buf[candidate_start..candidate_end].eq_ignore_ascii_case(needle)
            {
                let mut j = candidate_end;
                while j < buf.len() && (buf[j] == b' ' || buf[j] == b'\t') {
                    j += 1;
                }
                if j < buf.len() && buf[j] == b':' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

#[derive(Debug)]
pub enum HttpParseError {
    GetMoreData,
    Error(Box<StrOrString<'static>>),
    ReadingTimeout(Duration),
    Disconnected,
    InvalidHttpPayload(Box<StrOrString<'static>>),
}

impl HttpParseError {
    pub fn error(src: impl Into<StrOrString<'static>>) -> Self {
        HttpParseError::Error(Box::new(src.into()))
    }

    pub fn invalid_payload(src: impl Into<StrOrString<'static>>) -> Self {
        HttpParseError::InvalidHttpPayload(Box::new(src.into()))
    }

    pub fn get_more_data(&self) -> bool {
        matches!(self, HttpParseError::GetMoreData)
    }

    pub fn as_invalid_payload(&self) -> Option<&str> {
        match self {
            HttpParseError::InvalidHttpPayload(src) => Some(src.as_str()),
            _ => None,
        }
    }
}
