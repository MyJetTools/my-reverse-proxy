use tokio::io::ReadHalf;

use super::*;

mod parse_http_response_first_line;
pub use parse_http_response_first_line::*;
mod parse_http_header;
pub use parse_http_header::*;

pub async fn read_headers<TStream: tokio::io::AsyncRead>(
    read_stream: &mut ReadHalf<TStream>,
    tcp_buffer: &mut TcpBuffer,
    read_timeout: Duration,
    print_input_http_stream: bool,
) -> Result<BodyReader, HttpParseError> {
    let (status_code, version) = super::read_with_timeout::read_until_crlf(
        read_stream,
        tcp_buffer,
        read_timeout,
        parse_http_response_first_line,
        print_input_http_stream,
    )
    .await?;

    let mut builder = http::response::Builder::new()
        .status(status_code)
        .version(version);

    let mut detected_body_size = DetectedBodySize::Unknown;
    let mut headers_count: usize = 0;

    loop {
        let result = match tcp_buffer.read_until_crlf() {
            Some(line) => {
                if line.is_empty() {
                    break;
                }
                headers_count += 1;
                if headers_count > super::MAX_RESPONSE_HEADERS_COUNT {
                    return Err(HttpParseError::invalid_payload(format!(
                        "Response has more than {} headers",
                        super::MAX_RESPONSE_HEADERS_COUNT
                    )));
                }
                parse_http_header(builder, line)?
            }
            None => {
                super::read_with_timeout::read_to_buffer(
                    read_stream,
                    tcp_buffer,
                    read_timeout,
                    print_input_http_stream,
                )
                .await?;
                continue;
            }
        };

        builder = result.0;

        if !result.1.is_unknown() {
            detected_body_size = result.1;
        }
    }

    match detected_body_size {
        DetectedBodySize::Unknown => Ok(BodyReader::LengthBased {
            builder,
            body_size: 0,
        }),
        DetectedBodySize::Known(body_size) => Ok(BodyReader::LengthBased { builder, body_size }),
        DetectedBodySize::Chunked => {
            let (sender, response) = create_chunked_body_response(builder);
            Ok(BodyReader::Chunked { response, sender })
        }
        DetectedBodySize::WebSocketUpgrade => Ok(BodyReader::WebSocketUpgrade(
            WebSocketUpgradeBuilder::new(builder),
        )),
    }
}
