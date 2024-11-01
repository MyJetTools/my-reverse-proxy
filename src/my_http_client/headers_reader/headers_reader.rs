use crate::my_http_client::{
    BodyReader, BodyReaderChunked, BodyReaderLengthBased, DetectedBodySize, HttpParseError,
    TcpBuffer,
};

pub struct HeadersReader {
    builder: Option<http::response::Builder>,
    first_line_to_read: bool,
    detected_body_size: DetectedBodySize,
}

impl HeadersReader {
    pub fn new() -> Self {
        Self {
            builder: http::response::Builder::new().into(),
            first_line_to_read: true,
            detected_body_size: DetectedBodySize::Unknown,
        }
    }

    pub fn read(&mut self, tcp_buffer: &mut TcpBuffer) -> Result<BodyReader, HttpParseError> {
        if self.first_line_to_read {
            let first_line = tcp_buffer.read_until_crlf()?;

            let first_line = match std::str::from_utf8(first_line) {
                Ok(first_line) => first_line,
                Err(_) => {
                    return Err(HttpParseError::Error(
                        "Failed to parse first line. Invalid Utf8 Line".into(),
                    ))
                }
            };

            let (status_code, version) = super::parse_http_response_first_line(first_line)?;
            let builder = self.builder.take().unwrap();
            self.builder = builder.status(status_code).version(version).into();
            self.first_line_to_read = false;
        }

        loop {
            let header_line = tcp_buffer.read_until_crlf()?;

            if header_line.is_empty() {
                break;
            }

            let builder = self.builder.take().unwrap();
            let (builder, detected_body_size) = super::parse_http_header(builder, header_line)?;

            if !detected_body_size.is_unknown() {
                self.detected_body_size = detected_body_size;
            }

            self.builder = builder.into();
        }

        match self.detected_body_size {
            DetectedBodySize::Unknown => {
                let body_reader = BodyReaderLengthBased::new(self.builder.take().unwrap(), 0);
                return Ok(BodyReader::LengthBased(body_reader));
            }
            DetectedBodySize::Known(body_size) => {
                let body_reader =
                    BodyReaderLengthBased::new(self.builder.take().unwrap(), body_size);
                return Ok(BodyReader::LengthBased(body_reader));
            }
            DetectedBodySize::Chunked => {
                let body_reader = BodyReaderChunked::new(self.builder.take().unwrap());
                return Ok(BodyReader::Chunked(body_reader));
            }
            DetectedBodySize::WebSocketUpgrade => {
                return Ok(BodyReader::WebSocketUpgrade(self.builder.take()));
            }
        }
    }
}
