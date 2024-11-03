use std::{sync::Arc, time::Duration};

use crate::my_http_client::TcpBuffer;

use super::{BodyReader, HeadersReader, HttpTask, MyHttpClientInner};
use tokio::io::{AsyncReadExt, ReadHalf};

const READ_TIMEOUT: Duration = Duration::from_secs(120);

pub enum ReadModel {
    Header(HeadersReader),
    Body(BodyReader),
}

pub async fn read_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
>(
    mut read: ReadHalf<TStream>,
    connection_id: u64,
    inner: Arc<MyHttpClientInner<TStream>>,
) {
    let mut tcp_buffer = TcpBuffer::new();

    let mut read_mode = ReadModel::Header(HeadersReader::new());

    let mut do_read_to_buffer = true;

    while inner.is_my_connection_id(connection_id).await {
        if do_read_to_buffer {
            let result = read_to_buffer(&mut read, &mut tcp_buffer).await;
            if result.is_none() {
                break;
            }

            do_read_to_buffer = false;
        }

        match &mut read_mode {
            ReadModel::Header(headers_reader) => match headers_reader.read(&mut tcp_buffer) {
                Ok(mut body_reader) => {
                    if let Some(upgrade_response) = body_reader.try_into_web_socket_upgrade() {
                        let request = inner.pop_request(connection_id).await;
                        if let Some(mut request) = request {
                            request.set_ok(HttpTask::WebsocketUpgrade {
                                response: upgrade_response,
                                read_part: read,
                            });
                        }

                        return;
                    }

                    read_mode = ReadModel::Body(body_reader);
                }
                Err(err) => match err {
                    super::HttpParseError::GetMoreData => {
                        do_read_to_buffer = true;
                    }
                    super::HttpParseError::Error(err) => {
                        println!("Http parser error: {}", err);
                        break;
                    }
                },
            },
            ReadModel::Body(body_reader) => {
                let response = body_reader.try_extract_response(&mut tcp_buffer);

                match response {
                    Ok(response) => {
                        let request = inner.pop_request(connection_id).await;
                        if let Some(mut request) = request {
                            request.set_ok(HttpTask::Response(response));
                        } else {
                            println!("No request for response. Looks like it was a disconnect");
                            break;
                        }
                        read_mode = ReadModel::Header(HeadersReader::new());
                    }
                    Err(err) => match err {
                        super::HttpParseError::GetMoreData => {
                            do_read_to_buffer = true;
                        }
                        super::HttpParseError::Error(err) => {
                            println!("Http parser error: {}", err);
                            break;
                        }
                    },
                }
            }
        }
    }

    inner.disconnect(connection_id).await;
    println!("Http client read task is done");
}

async fn read_to_buffer<TStream: tokio::io::AsyncRead>(
    read: &mut ReadHalf<TStream>,
    tcp_buffer: &mut TcpBuffer,
) -> Option<usize> {
    let write_buf = tcp_buffer.get_write_buf();

    if write_buf.len() == 0 {
        println!("Http Payload is too big");
        return None;
    }

    let result = tokio::time::timeout(READ_TIMEOUT, read.read(write_buf)).await;

    if result.is_err() {
        println!("Http client Read timeout");
        return None;
    }

    let result = result.unwrap();

    if let Err(err) = result {
        println!("Http client Read error: {:?}", err);
        return None;
    }

    let result = result.unwrap();

    if result == 0 {
        println!("Http client Read EOF");
        return None;
    }

    /*
    println!(
        "Read: [{}]",
        std::str::from_utf8(&write_buf[..result]).unwrap()
    );
     */

    tcp_buffer.add_read_amount(result);

    Some(result)
}
