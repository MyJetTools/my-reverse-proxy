use super::*;
use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

pub struct HeaderPosition {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HttpContentLength {
    None,
    Known(usize),
    Chunked,
}

pub struct HttpHeaders {
    pub first_line_end: usize,
    pub end: usize,
    pub content_length: HttpContentLength,
    pub host_value: Option<HeaderPosition>,
}

impl HttpHeaders {
    pub fn parse(src: &[u8]) -> Option<Self> {
        const CL_CR: &[u8] = b"\r\n";
        const HOST_PREFIX: &[u8] = b"host:";

        let first_line_end = src.find_sequence_pos(CL_CR, 0)?;

        // Verify HTTP/1.1 first line
        if let Err(err) = verify_http11_first_line(&src[..first_line_end]) {
            eprintln!("Invalid HTTP/1.1 first line: {}", err);
            return None;
        }

        let mut host_value = None;
        let mut content_length = HttpContentLength::None;

        let mut header_start_pos = first_line_end + CL_CR.len();
        loop {
            let end = src.find_sequence_pos(CL_CR, header_start_pos)?;

            if end == header_start_pos {
                return Some(Self {
                    first_line_end,
                    host_value,
                    content_length,
                    end: end + CL_CR.len(),
                });
            }

            if host_value.is_none() {
                host_value = get_header_value(HOST_PREFIX, src, header_start_pos, end);
            }

            if matches!(content_length, HttpContentLength::None) {
                content_length = get_content_length(src, header_start_pos, end);
            }

            header_start_pos = end + CL_CR.len();
        }
    }

    pub fn get_first_line<'s>(&self, src: &'s [u8]) -> H1HeadersFirstLine<'s> {
        H1HeadersFirstLine {
            data: &src[..self.first_line_end],
        }
    }
}

fn get_header_value(
    header_prefix: &[u8],
    buf: &[u8],
    pos_start: usize,
    pos_end: usize,
) -> Option<HeaderPosition> {
    if pos_end - pos_start <= header_prefix.len() {
        return None;
    }

    if check_case_insensitive(
        &buf[pos_start..pos_start + header_prefix.len()],
        header_prefix,
    ) {
        let value_start = pos_start + header_prefix.len();
        let value_end = pos_end;

        // Trim leading whitespace
        let mut trimmed_start = value_start;
        while trimmed_start < value_end
            && (buf[trimmed_start] == b' ' || buf[trimmed_start] == b'\t')
        {
            trimmed_start += 1;
        }

        // Trim trailing whitespace
        let mut trimmed_end = value_end;
        while trimmed_end > trimmed_start
            && (buf[trimmed_end - 1] == b' ' || buf[trimmed_end - 1] == b'\t')
        {
            trimmed_end -= 1;
        }

        return Some(HeaderPosition {
            start: trimmed_start,
            end: trimmed_end,
        });
    }

    None
}

fn get_content_length(buf: &[u8], pos_start: usize, pos_end: usize) -> HttpContentLength {
    const CONTENT_LENGTH_PREFIX: &[u8] = b"content-length:";
    const TRANSFER_ENCODING_PREFIX: &[u8] = b"transfer-encoding:";

    // Check for Transfer-Encoding: chunked first
    if let Some(header_position) =
        get_header_value(TRANSFER_ENCODING_PREFIX, buf, pos_start, pos_end)
    {
        let value_bytes = &buf[header_position.start..header_position.end];
        if let Ok(value_str) = std::str::from_utf8(value_bytes) {
            let trimmed = value_str.trim().to_lowercase();
            if trimmed == "chunked" {
                return HttpContentLength::Chunked;
            }
        }
    }

    // Check for Content-Length header
    if let Some(header_position) = get_header_value(CONTENT_LENGTH_PREFIX, buf, pos_start, pos_end)
    {
        // Extract the value part and parse it
        let value_bytes = &buf[header_position.start..header_position.end];
        if let Ok(value_str) = std::str::from_utf8(value_bytes) {
            let trimmed = value_str.trim();
            if let Ok(length) = trimmed.parse::<usize>() {
                return HttpContentLength::Known(length);
            }
        }
    }

    HttpContentLength::None
}

fn check_case_insensitive(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    for (l, r) in left.iter().zip(right.iter()) {
        if l == r {
            continue;
        }

        // Convert to lowercase for comparison
        let l_lower = if *l >= b'A' && *l <= b'Z' {
            *l + 32 // Convert to lowercase
        } else {
            *l
        };

        let r_lower = if *r >= b'A' && *r <= b'Z' {
            *r + 32 // Convert to lowercase
        } else {
            *r
        };

        if l_lower != r_lower {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_length_parsing() {
        // Test with Content-Length header
        let http_request =
            b"POST /api/data HTTP/1.1\r\nContent-Length: 1024\r\nHost: example.com\r\n\r\n";
        let headers = HttpHeaders::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Known(1024));
    }

    #[test]
    fn test_content_length_case_insensitive() {
        // Test with case-insensitive Content-Length header
        let http_request =
            b"POST /api/data HTTP/1.1\r\ncontent-length: 2048\r\nHost: example.com\r\n\r\n";
        let headers = HttpHeaders::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Known(2048));
    }

    #[test]
    fn test_content_length_with_whitespace() {
        // Test with Content-Length header that has whitespace
        let http_request =
            b"POST /api/data HTTP/1.1\r\nContent-Length:  512  \r\nHost: example.com\r\n\r\n";
        let headers = HttpHeaders::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Known(512));
    }

    #[test]
    fn test_no_content_length_header() {
        // Test without Content-Length header
        let http_request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let headers = HttpHeaders::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::None);
    }

    #[test]
    fn test_chunked_transfer_encoding() {
        // Test with Transfer-Encoding: chunked header
        let http_request =
            b"POST /api/data HTTP/1.1\r\nTransfer-Encoding: chunked\r\nHost: example.com\r\n\r\n";
        let headers = HttpHeaders::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Chunked);
    }

    #[test]
    fn test_chunked_transfer_encoding_case_insensitive() {
        // Test with case-insensitive Transfer-Encoding: chunked header
        let http_request =
            b"POST /api/data HTTP/1.1\r\ntransfer-encoding: CHUNKED\r\nHost: example.com\r\n\r\n";
        let headers = HttpHeaders::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Chunked);
    }
}
