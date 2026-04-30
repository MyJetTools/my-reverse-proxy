use std::time::Duration;

use tokio::io::{AsyncReadExt, ReadHalf};

use super::{HttpParseError, TcpBuffer};

pub async fn read_to_buffer<TStream: tokio::io::AsyncRead>(
    read: &mut ReadHalf<TStream>,
    tcp_buffer: &mut TcpBuffer,
    read_time_out: Duration,
    print_http_payload: bool,
) -> Result<(), HttpParseError> {
    let write_buf = match tcp_buffer.get_write_buf() {
        Some(write_buf) if !write_buf.is_empty() => write_buf,
        _ => {
            return Err(HttpParseError::invalid_payload(format!(
                "Write Buffer is too small to read http headers. Size: [{}]",
                tcp_buffer.get_total_buffer_size()
            )));
        }
    };

    let result = tokio::time::timeout(read_time_out, read.read(write_buf)).await;

    if result.is_err() {
        return Err(HttpParseError::ReadingTimeout(read_time_out));
    }

    let result = result.unwrap();

    match result {
        Ok(result) => {
            if result == 0 {
                return Err(HttpParseError::Disconnected);
            }

            tcp_buffer.add_read_amount(result);

            if print_http_payload {
                let buf = tcp_buffer.get_buf();
                println!("Resp: [{:?}]", std::str::from_utf8(buf));
            }

            Ok(())
        }
        Err(err) => Err(HttpParseError::error(err.to_string())),
    }
}

pub async fn read_exact<TStream: tokio::io::AsyncRead>(
    read_stream: &mut ReadHalf<TStream>,
    buffer_to_write: &mut [u8],
    read_timeout: Duration,
) -> Result<usize, HttpParseError> {
    let mut pos = 0;
    loop {
        let feature = read_stream.read(&mut buffer_to_write[pos..]);

        let result = tokio::time::timeout(read_timeout, feature).await;

        if result.is_err() {
            return Err(HttpParseError::ReadingTimeout(read_timeout));
        }

        match result.unwrap() {
            Ok(result) => {
                if result == 0 {
                    return Err(HttpParseError::Disconnected);
                }

                pos += result;

                if pos == buffer_to_write.len() {
                    return Ok(result);
                }
            }
            Err(err) => {
                return Err(HttpParseError::error(format!(
                    "Error reading exact buffer: {:?}",
                    err
                )))
            }
        }
    }
}

pub async fn skip_exactly<TStream: tokio::io::AsyncRead>(
    read_stream: &mut ReadHalf<TStream>,
    tcp_buffer: &mut TcpBuffer,
    size_to_skip: usize,
    read_timeout: Duration,
    print_input_http_stream: bool,
) -> Result<(), HttpParseError> {
    loop {
        match tcp_buffer.skip_exactly(size_to_skip) {
            Ok(()) => {
                return Ok(());
            }
            Err(HttpParseError::GetMoreData) => {
                read_to_buffer(
                    read_stream,
                    tcp_buffer,
                    read_timeout,
                    print_input_http_stream,
                )
                .await?;
            }
            Err(err) => return Err(err),
        }
    }
}

pub async fn read_until_crlf<TResult, TStream: tokio::io::AsyncRead>(
    read_stream: &mut ReadHalf<TStream>,
    tcp_buffer: &mut TcpBuffer,
    read_timeout: Duration,
    conversion: impl Fn(&[u8]) -> Result<TResult, HttpParseError>,
    print_input_http_stream: bool,
) -> Result<TResult, HttpParseError> {
    loop {
        match tcp_buffer.read_until_crlf() {
            Some(as_str) => return conversion(as_str),
            None => {
                read_to_buffer(
                    read_stream,
                    tcp_buffer,
                    read_timeout,
                    print_input_http_stream,
                )
                .await?;
            }
        }
    }
}
