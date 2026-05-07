use std::time::Duration;

use bytes::Bytes;

use http_body_util::{BodyExt, StreamBody};
use tokio::io::ReadHalf;

use crate::http1::{HttpParseError, TcpBuffer, MAX_CHUNK_SIZE};

#[derive(Debug, Clone, Copy)]
pub enum ChunksReadingMode {
    WaitingFroChunkSize,
    ReadingChunk(usize),
    WaitingForSeparator,
    WaitingForEnd,
}

pub type ChunksSender =
    tokio::sync::mpsc::Sender<Result<hyper::body::Frame<Bytes>, hyper::Error>>;

pub fn create_chunked_body_response(
    builder: http::response::Builder,
) -> (ChunksSender, crate::HyperResponse) {
    let (sender, receiver) = tokio::sync::mpsc::channel(1024);
    let stream_body = StreamBody::new(tokio_stream::wrappers::ReceiverStream::new(receiver));

    let boxed_body = stream_body.map_err(|e: hyper::Error| e.to_string()).boxed();

    let chunked_body_response = builder.body(boxed_body).unwrap();
    (sender, chunked_body_response)
}

pub async fn read_chunked_body<TStream: tokio::io::AsyncRead>(
    read_stream: &mut ReadHalf<TStream>,
    tcp_buffer: &mut TcpBuffer,
    sender: tokio::sync::mpsc::Sender<Result<hyper::body::Frame<Bytes>, hyper::Error>>,
    read_timeout: Duration,
    print_input_http_stream: bool,
) -> Result<(), HttpParseError> {
    loop {
        let chunk_size = super::super::read_with_timeout::read_until_crlf(
            read_stream,
            tcp_buffer,
            read_timeout,
            parse_chunk_size,
            print_input_http_stream,
        )
        .await?;

        if print_input_http_stream {
            println!("Read body chunk size: {}", chunk_size);
        }

        if chunk_size == 0 {
            super::super::read_with_timeout::skip_exactly(
                read_stream,
                tcp_buffer,
                2,
                read_timeout,
                print_input_http_stream,
            )
            .await?;

            return Ok(());
        }

        if chunk_size > MAX_CHUNK_SIZE {
            return Err(HttpParseError::invalid_payload(format!(
                "Chunk size {} exceeds limit {}",
                chunk_size, MAX_CHUNK_SIZE
            )));
        }

        let mut chunk: Vec<u8> = vec![0u8; chunk_size];

        let mut read_amount = 0;

        if let Some(remains_in_buffer) = tcp_buffer.get_as_much_as_possible(chunk_size) {
            chunk[..remains_in_buffer.len()].copy_from_slice(remains_in_buffer);
            read_amount += remains_in_buffer.len();
        }

        let remains_to_read = chunk_size - read_amount;

        if remains_to_read > 0 {
            super::super::read_with_timeout::read_exact(
                read_stream,
                &mut chunk[read_amount..],
                read_timeout,
            )
            .await?;
        }

        let err = sender
            .send(Ok(hyper::body::Frame::data(chunk.into())))
            .await;

        if let Err(err) = err {
            return Err(HttpParseError::error(format!(
                "Error sending response chunk: {:?}",
                err
            )));
        }

        super::super::read_with_timeout::skip_exactly(
            read_stream,
            tcp_buffer,
            2,
            read_timeout,
            print_input_http_stream,
        )
        .await?;
    }
}

fn parse_chunk_size(src: &[u8]) -> Result<usize, HttpParseError> {
    let mut end_of_hex = src.len();

    for (i, &byte) in src.iter().enumerate() {
        if !byte.is_ascii_hexdigit() {
            end_of_hex = i;
            break;
        }
    }

    if end_of_hex == 0 {
        return Err(HttpParseError::invalid_payload(format!(
            "Invalid chunk size: {:?}",
            std::str::from_utf8(src).unwrap()
        )));
    }

    let hex_str = std::str::from_utf8(&src[0..end_of_hex])
        .map_err(|_| HttpParseError::invalid_payload("Invalid UTF-8 in chunk size"))?;

    usize::from_str_radix(hex_str, 16).map_err(|_| {
        HttpParseError::invalid_payload(format!("Can not parse chunk size: {}", hex_str))
    })
}
