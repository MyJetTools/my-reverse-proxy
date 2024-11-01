use http::{StatusCode, Version};

use super::super::HttpParseError;

pub fn parse_http_response_first_line(src: &str) -> Result<(StatusCode, Version), HttpParseError> {
    let mut lines = src.split(' ');

    let protocol = lines.next();

    if protocol.is_none() {
        return Err(HttpParseError::Error(format!("Invalid Line {}", src)));
    }
    let protocol = protocol.unwrap();

    let status_code = lines.next();

    if status_code.is_none() {
        return Err(HttpParseError::Error(format!("Invalid Line {}", src)));
    }

    let version = match protocol {
        "HTTP/1.0" => http::Version::HTTP_10,
        "HTTP/1.1" => http::Version::HTTP_11,
        _ => return Err(HttpParseError::Error("Invalid protocol".to_string())),
    };

    let status_code = status_code.unwrap();

    let status_code: StatusCode = status_code.parse().unwrap();

    Ok((status_code, version))
}
