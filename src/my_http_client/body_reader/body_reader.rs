use bytes::Bytes;
use http_body_util::combinators::BoxBody;

use super::*;

#[derive(Debug)]
pub enum BodyReader {
    LengthBased(FullBodyReader),
    Chunked(BodyReaderChunked),
    WebSocketUpgrade(WebSocketUpgradeBuilder),
}

#[derive(Debug)]
pub struct WebSocketUpgradeBuilder {
    builder: Option<http::response::Builder>,
}

impl WebSocketUpgradeBuilder {
    pub fn new(builder: http::response::Builder) -> Self {
        Self {
            builder: Some(builder),
        }
    }

    pub fn take_upgrade_response(&mut self) -> http::Response<BoxBody<Bytes, String>> {
        let builder = self.builder.take();
        if builder.is_none() {
            panic!("WebSocket upgrade response is already taken");
        }

        crate::utils::into_empty_body(builder.unwrap())
    }
}

/*
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
     */

/*
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
     */
