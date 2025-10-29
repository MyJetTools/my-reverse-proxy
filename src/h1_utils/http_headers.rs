use super::*;
use rust_extensions::{slice_of_u8_utils::SliceOfU8Ext, str_utils::StrUtils};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HttpContentLength {
    None,
    Known(usize),
    Chunked,
}

pub struct Http1Headers {
    pub first_line_end: usize,
    pub end: usize,
    pub content_length: HttpContentLength,
    pub host_value: Option<HeaderPosition>,
    pub cookie_value: Option<HeaderPosition>,
}

impl Http1Headers {
    pub fn parse(src: &[u8]) -> Option<Self> {
        const HOST_HEADER: &[u8] = b"host";
        const COOKIE_HEADER: &[u8] = b"cookie";
        const CONTENT_LEN_HEADER: &[u8] = b"content-length";
        const TRANSFER_ENCODING_HEADER: &[u8] = b"transfer-encoding";

        let first_line_end = src.find_sequence_pos(crate::consts::HTTP_CR_LF, 0)?;

        // Verify HTTP/1.1 first line
        if let Err(err) = verify_http11_first_line(&src[..first_line_end]) {
            eprintln!("Invalid HTTP/1.1 first line: {}", err);
            return None;
        }

        let mut host_value = None;
        let mut cookie_value = None;
        let mut content_length = HttpContentLength::None;

        let mut header_start_pos = first_line_end + crate::consts::HTTP_CR_LF.len();
        loop {
            let end = src.find_sequence_pos(crate::consts::HTTP_CR_LF, header_start_pos)?;

            if end == header_start_pos {
                return Some(Self {
                    first_line_end,
                    host_value,
                    cookie_value,
                    content_length,
                    end: end + crate::consts::HTTP_CR_LF.len(),
                });
            }

            let http_header = HttpHeader::new(src, header_start_pos, end)?;
            if http_header.is_my_header_name(HOST_HEADER) {
                host_value = Some(http_header.get_value());
            } else if http_header.is_my_header_name(COOKIE_HEADER) {
                cookie_value = Some(http_header.get_value());
            } else if http_header.is_my_header_name(CONTENT_LEN_HEADER) {
                content_length = HttpContentLength::Known(http_header.get_usize_value()?);
            } else if http_header.is_my_header_name(TRANSFER_ENCODING_HEADER) {
                let value = http_header.get_value_as_str()?;
                if value.eq_case_insensitive("chunked") {
                    content_length = HttpContentLength::Chunked;
                }
            }

            header_start_pos = end + crate::consts::HTTP_CR_LF.len();
        }
    }

    pub fn get_first_line<'s>(&self, src: &'s [u8]) -> H1HeadersFirstLine<'s> {
        H1HeadersFirstLine {
            data: &src[..self.first_line_end],
        }
    }
}

/*
fn get_header_value<'s>(
    buf: &'s [u8],
    pos_start: usize,
    pos_end: usize,
) -> Result<(&'s [u8], Option<HeaderPosition>), ProxyPassError> {


    let Some(header_index) = header_index else{
        return Err(ProxyPassErr)
    };

    let key = buf[..header_index]


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
 */
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_length_parsing() {
        // Test with Content-Length header
        let http_request =
            b"POST /api/data HTTP/1.1\r\nContent-Length: 1024\r\nHost: example.com\r\n\r\n";
        let headers = Http1Headers::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Known(1024));
    }

    #[test]
    fn test_content_length_case_insensitive() {
        // Test with case-insensitive Content-Length header
        let http_request =
            b"POST /api/data HTTP/1.1\r\ncontent-length: 2048\r\nHost: example.com\r\n\r\n";
        let headers = Http1Headers::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Known(2048));
    }

    #[test]
    fn test_content_length_with_whitespace() {
        // Test with Content-Length header that has whitespace
        let http_request =
            b"POST /api/data HTTP/1.1\r\nContent-Length:  512  \r\nHost: example.com\r\n\r\n";
        let headers = Http1Headers::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Known(512));
    }

    #[test]
    fn test_no_content_length_header() {
        // Test without Content-Length header
        let http_request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let headers = Http1Headers::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::None);
    }

    #[test]
    fn test_chunked_transfer_encoding() {
        // Test with Transfer-Encoding: chunked header
        let http_request =
            b"POST /api/data HTTP/1.1\r\nTransfer-Encoding: chunked\r\nHost: example.com\r\n\r\n";
        let headers = Http1Headers::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Chunked);
    }

    #[test]
    fn test_chunked_transfer_encoding_case_insensitive() {
        // Test with case-insensitive Transfer-Encoding: chunked header
        let http_request =
            b"POST /api/data HTTP/1.1\r\ntransfer-encoding: CHUNKED\r\nHost: example.com\r\n\r\n";
        let headers = Http1Headers::parse(http_request);
        assert!(headers.is_some());
        let headers = headers.unwrap();
        assert_eq!(headers.content_length, HttpContentLength::Chunked);
    }
}
