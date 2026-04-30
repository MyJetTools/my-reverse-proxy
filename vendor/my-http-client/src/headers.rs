pub trait MyHttpClientHeaders {
    fn copy_to(&self, buf: &mut Vec<u8>);
}

pub struct HeaderValuePosition {
    pub start: usize,
    pub end: usize,
}

pub struct MyHttpClientHeadersBuilder {
    headers: Vec<u8>,
}

impl Default for MyHttpClientHeadersBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MyHttpClientHeadersBuilder {
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
        }
    }

    pub fn add_header(&mut self, name: &str, value: &str) -> HeaderValuePosition {
        write_header(&mut self.headers, name, value)
    }

    pub fn get_value(&self, value_position: &HeaderValuePosition) -> &str {
        unsafe {
            std::str::from_utf8_unchecked(&self.headers[value_position.start..value_position.end])
        }
    }

    pub fn iter(&self) -> MyHttpClientHeadersBuilderIterator<'_> {
        MyHttpClientHeadersBuilderIterator::new(&self.headers)
    }

    pub fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.headers) }
    }
}

impl MyHttpClientHeaders for MyHttpClientHeadersBuilder {
    fn copy_to(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.headers);
    }
}

pub struct MyHttpClientHeadersBuilderIterator<'s> {
    itm: &'s [u8],
    pos: usize,
}

impl<'s> MyHttpClientHeadersBuilderIterator<'s> {
    pub fn new(itm: &'s [u8]) -> Self {
        Self { itm, pos: 0 }
    }
}

impl<'s> Iterator for MyHttpClientHeadersBuilderIterator<'s> {
    type Item = (&'s str, &'s str);

    fn next(&mut self) -> Option<Self::Item> {
        let header_start = self.pos;

        let header_end;

        loop {
            if self.pos == self.itm.len() {
                return None;
            }
            if self.itm[self.pos] == b':' {
                header_end = self.pos;
                break;
            }
            self.pos += 1;
        }

        self.pos += 2;

        let value_start = self.pos;
        let value_end;

        loop {
            if self.pos >= self.itm.len() {
                return None;
            }
            if self.itm[self.pos] == b'\r' {
                value_end = self.pos;
                break;
            }
            self.pos += 1;
        }
        self.pos += 2;

        (
            std::str::from_utf8(&self.itm[header_start..header_end]).unwrap(),
            std::str::from_utf8(&self.itm[value_start..value_end]).unwrap(),
        )
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::MyHttpClientHeadersBuilder;

    #[test]
    fn test_iterators() {
        let mut headers = MyHttpClientHeadersBuilder::new();

        headers.add_header("Content-Type", "text/plain");
        headers.add_header("Content-Length", "123");

        let mut iter = headers.iter();
        let (name, value) = iter.next().unwrap();
        assert_eq!(name, "Content-Type");
        assert_eq!(value, "text/plain");

        let (name, value) = iter.next().unwrap();
        assert_eq!(name, "Content-Length");
        assert_eq!(value, "123");

        assert!(iter.next().is_none());
    }
}

pub fn validate_header_name(name: &str) {
    if name.is_empty() {
        panic!("HTTP header name must not be empty");
    }
    for &b in name.as_bytes() {
        if !is_valid_header_name_byte(b) {
            panic!("HTTP header name contains forbidden byte 0x{:02x}", b);
        }
    }
}

pub fn validate_header_value(value: &str) {
    for &b in value.as_bytes() {
        if b == b'\r' || b == b'\n' || b == 0 {
            panic!(
                "HTTP header value contains forbidden control byte 0x{:02x} (header injection)",
                b
            );
        }
    }
}

fn is_valid_header_name_byte(b: u8) -> bool {
    matches!(
        b,
        b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.'
        | b'^' | b'_' | b'`' | b'|' | b'~'
        | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z'
    )
}

pub fn write_header(dest: &mut Vec<u8>, name: &str, value: &str) -> HeaderValuePosition {
    validate_header_name(name);
    validate_header_value(value);
    dest.extend_from_slice(name.as_bytes());
    dest.extend_from_slice(": ".as_bytes());
    let start = dest.len();
    dest.extend_from_slice(value.as_bytes());
    let end = dest.len();
    dest.extend_from_slice(crate::CL_CR);
    HeaderValuePosition { start, end }
}
