use std::{sync::Arc, time::Duration};

use super::{HttpParseError, TcpBuffer};

use super::{BodyReader, HttpTask, MyHttpClientInner};
use tokio::io::ReadHalf;

pub async fn read_loop<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
>(
    mut read_stream: ReadHalf<TStream>,
    connection_id: u64,
    inner: Arc<MyHttpClientInner<TStream>>,
    read_timeout: Duration,
) -> Result<(), HttpParseError> {
    let mut do_read_to_buffer = true;

    let mut tcp_buffer = TcpBuffer::new();

    let print_input_http_stream = if let Ok(value) = std::env::var("DEBUG_HTTP_INPUT_STREAM") {
        println!("http_client_name: {}", inner.name.as_str());
        value.as_str() == inner.name.as_str()
    } else {
        false
    };
    while inner.is_my_connection_id(connection_id) {
        if do_read_to_buffer || tcp_buffer.is_empty() {
            super::read_with_timeout::read_to_buffer(
                &mut read_stream,
                &mut tcp_buffer,
                read_timeout,
                print_input_http_stream,
            )
            .await?;

            do_read_to_buffer = false;
        }

        match super::headers_reader::read_headers(
            &mut read_stream,
            &mut tcp_buffer,
            read_timeout,
            print_input_http_stream,
        )
        .await
        {
            Ok(body_reader) => match body_reader {
                BodyReader::LengthBased { builder, body_size } => {
                    let response = super::body_reader::read_full_body(
                        &mut read_stream,
                        &mut tcp_buffer,
                        builder,
                        body_size,
                        read_timeout,
                    )
                    .await?;

                    let request = inner.pop_request(connection_id, false);
                    if let Some(mut request) = request {
                        let result = request.try_set_ok(HttpTask::Response(response));

                        if result.is_err() {
                            return Ok(());
                        }
                    }
                }
                BodyReader::Chunked { response, sender } => {
                    let request = inner.pop_request(connection_id, false);
                    if let Some(mut request) = request {
                        let result = request.try_set_ok(HttpTask::Response(response));

                        if result.is_err() {
                            return Ok(());
                        }
                    }

                    super::body_reader::read_chunked_body(
                        &mut read_stream,
                        &mut tcp_buffer,
                        sender,
                        read_timeout,
                        print_input_http_stream,
                    )
                    .await?;
                }
                BodyReader::WebSocketUpgrade(mut builder) => {
                    let upgrade_response = builder.take_upgrade_response();
                    let request = inner.pop_request(connection_id, true);
                    if let Some(mut request) = request {
                        let _ = request.try_set_ok(HttpTask::WebsocketUpgrade {
                            response: upgrade_response,
                            read_part: read_stream,
                        });
                    }

                    return Ok(());
                }
            },
            Err(err) => match err {
                super::HttpParseError::GetMoreData => {
                    do_read_to_buffer = true;
                }
                _ => {
                    return Err(err);
                }
            },
        }
    }

    Ok(())
}
