use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use crate::h1_utils::H1HeadersFirstLine;

pub struct Http1HeadersBuilder {
    payload: Vec<u8>,
}

impl Http1HeadersBuilder {
    pub fn new() -> Self {
        Self {
            payload: Vec::new(),
        }
    }

    pub fn push_response_first_line(&mut self, status_code: u16) {
        match status_code {
            200 => {
                self.payload.extend_from_slice(b"HTTP/1.1 200 OK");
            }
            401 => {
                self.payload.extend_from_slice(b"HTTP/1.1 401 Unauthorized");
            }
            502 => {
                self.payload.extend_from_slice(b"HTTP/1.1 502 Bad Gateway");
            }
            _ => {
                self.payload
                    .extend_from_slice(b"HTTP/1.1 503 Service Temporarily Unavailable");
            }
        }

        self.push_cl_cr();
    }

    pub fn push_header(&mut self, name: &str, value: &str) {
        self.payload.extend_from_slice(name.as_bytes());
        self.payload.extend_from_slice(": ".as_bytes());
        self.payload.extend_from_slice(value.as_bytes());
        self.push_cl_cr();
    }

    pub fn push_content_length(&mut self, size: usize) {
        self.push_header("content-length", size.to_string().as_str());
    }

    pub fn push_cl_cr(&mut self) {
        self.payload.extend_from_slice(crate::consts::HTTP_CR_LF);
    }

    pub fn push_raw_payload(&mut self, buffer: &[u8]) {
        self.payload.extend_from_slice(buffer);
    }

    pub fn push_space(&mut self) {
        self.payload.push(b' ');
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.payload
    }

    pub fn clear(&mut self) {
        self.payload.clear();
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.payload
    }

    pub fn get_first_line<'s>(&'s self) -> H1HeadersFirstLine<'s> {
        let index = self.payload.find_byte_pos(b'\n', 0).unwrap();

        H1HeadersFirstLine {
            data: &self.payload[..index],
        }
    }
}
