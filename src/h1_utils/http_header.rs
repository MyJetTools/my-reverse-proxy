use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;
#[derive(Clone, Copy)]
pub struct HeaderPosition {
    pub start: usize,
    pub end: usize,
}

pub struct HttpHeader<'s> {
    payload: &'s [u8],
    header_start: usize,
    header_separator_index: usize,
    header_end: usize,
}

impl<'s> HttpHeader<'s> {
    pub fn new(payload: &'s [u8], header_start_pos: usize, header_end: usize) -> Option<Self> {
        let header_separator_index = payload.find_byte_pos(b':', header_start_pos)?;

        Self {
            payload,
            header_start: header_start_pos,
            header_separator_index,
            header_end,
        }
        .into()
    }
    pub fn is_my_header_name(&self, value: &[u8]) -> bool {
        if self.header_separator_index - self.header_start != value.len() {
            return false;
        }

        let header_name = &self.payload[self.header_start..self.header_separator_index];

        compare_case_insensitive(header_name, value)
    }

    pub fn get_value_as_str(&self) -> Option<&str> {
        let value = &self.payload[self.header_separator_index + 2..self.header_end];

        match std::str::from_utf8(value) {
            Ok(value) => Some(value.trim()),
            Err(_) => None,
        }
    }

    pub fn get_value(&self) -> HeaderPosition {
        HeaderPosition {
            start: self.header_separator_index + 1,
            end: self.header_end,
        }
    }

    pub fn get_usize_value(&self) -> Option<usize> {
        let value = self.get_value_as_str()?;
        match value.parse() {
            Ok(result) => Some(result),
            Err(_) => None,
        }
    }
}

pub fn compare_case_insensitive(left: &[u8], right: &[u8]) -> bool {
    let mut h_iter = left.iter();
    for v in right {
        let p = h_iter.next().unwrap();

        if v == p {
            continue;
        }

        let v = to_lower_case(*v);
        let p = to_lower_case(*p);

        if v != p {
            return false;
        }
    }

    true
}

fn to_lower_case(b: u8) -> u8 {
    if b >= b'A' && b <= b'Z' {
        return b - b'A' + b'a';
    }

    b
}

#[cfg(test)]
mod tests {
    use crate::h1_utils::http_header::to_lower_case;

    #[test]
    fn tests() {
        assert_eq!(to_lower_case(b'A'), b'a');

        assert_eq!(to_lower_case(b'a'), b'a');
        assert_eq!(to_lower_case(b'Z'), b'z');
    }
}
