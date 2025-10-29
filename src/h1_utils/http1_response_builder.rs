use crate::h1_utils::Http1HeadersBuilder;

pub struct Http1ResponseBuilder {
    headers: Http1HeadersBuilder,
}

impl Http1ResponseBuilder {
    pub fn new_as_ok_result() -> Self {
        Self::new(200)
    }

    pub fn new(status_code: u16) -> Self {
        let mut result = Self {
            headers: Http1HeadersBuilder::new(),
        };

        result.headers.add_response_first_line(status_code);

        result
    }

    pub fn add_content_type(mut self, value: &str) -> Self {
        self.headers.add_header("content-type", value);
        self
    }

    pub fn add_header(mut self, name: &str, value: &str) -> Self {
        self.headers.add_header(name, value);
        self
    }

    pub fn build_with_body(mut self, body: &[u8]) -> Vec<u8> {
        self.headers.add_content_length(body.len());
        self.headers.write_cl_cr();
        let mut result = self.headers.into_bytes();

        result.extend_from_slice(body);

        result
    }
}
