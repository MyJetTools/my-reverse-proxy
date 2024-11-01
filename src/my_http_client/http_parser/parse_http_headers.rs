use core::str;

use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use super::{
    super::{BodyReader, BodyReaderChunked, BodyReaderLengthBased},
    parse_http_response_first_line, DetectedBodySize, HttpParseError,
};

pub fn parse_http_headers(payload: &[u8]) -> Result<BodyReader, HttpParseError> {
    let mut builder = http::response::Builder::new();
    let pos = payload.find_byte_pos(b'\r', 0);

    if pos.is_none() {
        return Err(HttpParseError::GetMoreData);
    }
    let pos = pos.unwrap();

    if pos == payload.len() - 1 {
        return Err(HttpParseError::GetMoreData);
    }

    let http_line = &payload[..pos];

    let (status, version) = parse_http_response_first_line(str::from_utf8(http_line).unwrap())?;

    builder = builder.status(status).version(version);

    let mut pos = pos + 1;

    if payload[pos] != b'\n' {
        return Err(HttpParseError::Error("Invalid http payload".to_string()));
    }

    pos = pos + 1;

    let mut detected_body_size = DetectedBodySize::Unknown;

    loop {
        let next_pos = payload.find_byte_pos(b'\r', pos);

        if next_pos.is_none() {
            return Err(HttpParseError::GetMoreData);
        }

        let next_pos = next_pos.unwrap();

        if next_pos == pos {
            pos = next_pos + 1;
            if payload[pos] != b'\n' {
                return Err(HttpParseError::Error("Invalid http payload".to_string()));
            }
            pos += 1;
            break;
        }

        let response = super::parse_http_header(builder, &payload[pos..next_pos])?;

        builder = response.0;
        let body_size_from_header = response.1;

        if !body_size_from_header.is_unknown() {
            detected_body_size = body_size_from_header;
        }

        /*
        if header.eq_ignore_ascii_case("Content-Length") {
            match value.parse() {
                Ok(value) => detected_body_size = Some(value),
                Err(_) => {
                    return Err(HttpParseResult::Error(format!(
                        "Invalid Content-Length value: {}",
                        value
                    )));
                }
            }
        }
         */

        let next_pos = next_pos + 1;
        if payload[next_pos] != b'\n' {
            return Err(HttpParseError::Error("Invalid http payload".to_string()));
        }

        pos = next_pos + 1;
    }

    match detected_body_size {
        DetectedBodySize::Unknown => {
            return Err(HttpParseError::Error(
                "Content-Length or 'Transfer-Encoding: chunked' is not found".to_string(),
            ));
        }
        DetectedBodySize::Known(body_size) => {
            let body_reader = BodyReaderLengthBased::new(builder, pos, body_size);
            return Ok(BodyReader::LengthBased(body_reader));
        }
        DetectedBodySize::Chunked => {
            let body_reader = BodyReaderChunked::new(builder, pos);
            return Ok(BodyReader::Chunked(body_reader));
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_parse_http() {
        let payload = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";

        let result = super::parse_http_headers(payload).unwrap();

        println!("{:?}", result);
    }

    #[test]
    fn test_parse_http_2() {
        let payload = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: 139\r\nConnection: close\r\n\r\n<html>\n<head>\n<title>Sample Response</title>\n</head>\n<body>\n<h1>Hello, world!</h1>\n<p>This is an example HTTP response.</p>\n</body>\n</html>";

        let result = super::parse_http_headers(payload.as_bytes()).unwrap();

        println!("{:?}", result);
    }
}
