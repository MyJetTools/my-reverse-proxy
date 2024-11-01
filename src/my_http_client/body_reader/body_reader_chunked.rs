use bytes::Bytes;
use http_body_util::combinators::BoxBody;

use crate::my_http_client::{HttpParseError, TcpBuffer};

use super::BodyReaderInner;

#[derive(Debug, Clone, Copy)]
pub enum ChunksReadingMode {
    WaitingFroChunkSize,
    ReadingChunk(usize),
    WaitingForSeparator,
    WaitingForEnd,
}

#[derive(Debug)]
pub struct BodyReaderChunked {
    pub inner: Option<BodyReaderInner>,

    pub reading_mode: ChunksReadingMode,
}

impl BodyReaderChunked {
    pub fn new(builder: http::response::Builder) -> Self {
        let body = Vec::new();
        Self {
            inner: Some(BodyReaderInner { builder, body }),
            reading_mode: ChunksReadingMode::WaitingFroChunkSize,
        }
    }

    pub fn try_extract_response(
        &mut self,
        read_buffer: &mut TcpBuffer,
    ) -> Result<http::Response<BoxBody<Bytes, String>>, HttpParseError> {
        loop {
            match self.reading_mode {
                ChunksReadingMode::WaitingFroChunkSize => {
                    let chunk_size_str = read_buffer.read_until_crlf()?;

                    match get_chunk_size(chunk_size_str) {
                        Some(chunk_size) => {
                            if chunk_size == 0 {
                                self.reading_mode = ChunksReadingMode::WaitingForEnd;
                            } else {
                                self.reading_mode = ChunksReadingMode::ReadingChunk(chunk_size);
                            }
                        }
                        None => {
                            return Err(HttpParseError::Error(format!(
                                "Failed to parse chunk size. Invalid number [{:?}]",
                                std::str::from_utf8(chunk_size_str)
                            )));
                        }
                    }
                }
                ChunksReadingMode::ReadingChunk(chunk_size) => {
                    let buf = read_buffer.get_as_much_as_possible(chunk_size)?;

                    self.inner.as_mut().unwrap().body.extend_from_slice(buf);

                    let remains_to_read = chunk_size - buf.len();

                    if remains_to_read == 0 {
                        self.reading_mode = ChunksReadingMode::WaitingForSeparator;
                    } else {
                        self.reading_mode = ChunksReadingMode::ReadingChunk(remains_to_read);
                    }
                }
                ChunksReadingMode::WaitingForSeparator => {
                    read_buffer.skip_exactly(2)?;
                    self.reading_mode = ChunksReadingMode::WaitingFroChunkSize;
                }
                ChunksReadingMode::WaitingForEnd => {
                    read_buffer.skip_exactly(2)?;

                    let inner = self.inner.take().unwrap();

                    return Ok(inner.into_body(true));
                }
            }
        }
    }
}

fn get_chunk_size(src: &[u8]) -> Option<usize> {
    let mut result = 0;

    let mut i = src.len() - 1;

    let mut multiplier = 1;
    loop {
        let number = from_hex_number(src[i])?;

        result += number * multiplier;

        multiplier *= 16;
        if i == 0 {
            break;
        }

        i -= 1;
    }

    Some(result)
}

fn from_hex_number(c: u8) -> Option<usize> {
    match c {
        b'0'..=b'9' => Some((c - b'0') as usize),
        b'a'..=b'f' => Some((c - b'a' + 10) as usize),
        b'A'..=b'F' => Some((c - b'A' + 10) as usize),
        _ => None,
    }
}
