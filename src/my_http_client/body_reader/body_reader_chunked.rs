use bytes::Bytes;
use futures::SinkExt;
use http::Response;
use http_body_util::{combinators::BoxBody, BodyExt, StreamBody};

use crate::my_http_client::{HttpParseError, TcpBuffer};

#[derive(Debug, Clone, Copy)]
pub enum ChunksReadingMode {
    WaitingFroChunkSize,
    ReadingChunk(usize),
    WaitingForSeparator,
    WaitingForEnd,
}

#[derive(Debug)]
pub struct BodyReaderChunked {
    pub reading_mode: ChunksReadingMode,
    sender: futures::channel::mpsc::Sender<Result<hyper::body::Frame<Bytes>, hyper::Error>>,
    pub current_chunk: Option<Vec<u8>>,
    pub chunked_body_response: Option<Response<BoxBody<Bytes, String>>>,
}

impl BodyReaderChunked {
    pub fn new(builder: http::response::Builder) -> Self {
        let (sender, receiver) = futures::channel::mpsc::channel(1024);

        //   let chunk = hyper::body::Frame::data(vec![0u8].into());
        //   let send_result = sender.send(Ok(chunk)).await;

        let stream_body = StreamBody::new(receiver);

        let boxed_body = stream_body.map_err(|e: hyper::Error| e.to_string()).boxed();

        let chunked_body_response = builder.body(boxed_body).unwrap();

        Self {
            reading_mode: ChunksReadingMode::WaitingFroChunkSize,
            sender,
            current_chunk: None,
            chunked_body_response: chunked_body_response.into(),
        }
    }

    pub fn get_chunked_body_response(&mut self) -> Option<Response<BoxBody<Bytes, String>>> {
        self.chunked_body_response.take()
    }

    pub async fn populate_and_detect_last_body_chunk(
        &mut self,
        read_buffer: &mut TcpBuffer,
    ) -> Result<(), HttpParseError> {
        loop {
            match self.reading_mode {
                ChunksReadingMode::WaitingFroChunkSize => {
                    let chunk_size_str = read_buffer.read_until_crlf()?;

                    match get_chunk_size(chunk_size_str) {
                        Some(chunk_size) => {
                            if chunk_size == 0 {
                                self.reading_mode = ChunksReadingMode::WaitingForEnd;
                            } else {
                                self.current_chunk = Vec::with_capacity(chunk_size).into();
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

                    self.current_chunk.as_mut().unwrap().extend_from_slice(buf);

                    let remains_to_read = chunk_size - buf.len();

                    if remains_to_read == 0 {
                        let chunk = self.current_chunk.take().unwrap();

                        let _ = self
                            .sender
                            .send(Ok(hyper::body::Frame::data(chunk.into())))
                            .await;
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

                    return Ok(());
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
