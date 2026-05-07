use http::{StatusCode, Version};

use super::super::HttpParseError;

pub fn parse_http_response_first_line(src: &[u8]) -> Result<(StatusCode, Version), HttpParseError> {
    let src = std::str::from_utf8(src).map_err(|_| {
        HttpParseError::invalid_payload(
            "Invalid HTTP first line. Can not convert payload to UTF8 string",
        )
    })?;

    let mut lines = src.split(' ');

    let protocol_version = lines.next().ok_or_else(|| {
        HttpParseError::invalid_payload(format!("Invalid Http First Line: [{}]", src))
    })?;

    let protocol_version = match protocol_version {
        "HTTP/1.0" => http::Version::HTTP_10,
        "HTTP/1.1" => http::Version::HTTP_11,
        _ => {
            return Err(HttpParseError::invalid_payload(format!(
                "Not supported HTTP protocol. [{}].",
                protocol_version
            )));
        }
    };

    let status_code = lines.next().ok_or_else(|| {
        HttpParseError::invalid_payload(format!("Invalid Http First Line: [{}]", src))
    })?;

    let status_code = status_code.parse().map_err(|err| {
        HttpParseError::invalid_payload(format!(
            "Invalid HTTP status code [{}]. Err: {}",
            status_code, err
        ))
    })?;

    Ok((status_code, protocol_version))
}
