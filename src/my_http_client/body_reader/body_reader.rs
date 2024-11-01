use bytes::Bytes;
use http_body_util::combinators::BoxBody;

use crate::my_http_client::{HttpParseError, TcpBuffer};

use super::*;

#[derive(Debug)]
pub enum BodyReader {
    LengthBased(BodyReaderLengthBased),
    Chunked(BodyReaderChunked),
    WebSocketUpgrade(Option<http::response::Builder>),
}

impl BodyReader {
    pub fn try_extract_response(
        &mut self,
        tcp_buffer: &mut TcpBuffer,
    ) -> Result<http::Response<BoxBody<Bytes, String>>, HttpParseError> {
        match self {
            BodyReader::LengthBased(body_reader) => body_reader.try_extract_response(tcp_buffer),
            BodyReader::Chunked(body_reader) => body_reader.try_extract_response(tcp_buffer),
            BodyReader::WebSocketUpgrade(_) => {
                panic!("WebSocket upgrade is not supported to extract body")
            }
        }
    }

    pub fn try_into_web_socket_upgrade(
        &mut self,
    ) -> Option<http::Response<BoxBody<Bytes, String>>> {
        match self {
            BodyReader::WebSocketUpgrade(body_reader) => {
                let builder = body_reader.take()?;
                Some(crate::utils::into_empty_body(builder))
            }
            _ => None,
        }
    }
}
