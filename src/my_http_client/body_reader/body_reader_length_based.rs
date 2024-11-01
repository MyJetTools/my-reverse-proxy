use bytes::Bytes;
use http_body_util::combinators::BoxBody;

use super::*;
use crate::my_http_client::{HttpParseError, TcpBuffer};

#[derive(Debug)]
pub struct BodyReaderLengthBased {
    inner: Option<BodyReaderInner>,
    pub body_size: usize,
}

impl BodyReaderLengthBased {
    pub fn new(builder: http::response::Builder, body_size: usize) -> Self {
        let body = Vec::with_capacity(body_size);
        Self {
            inner: Some(BodyReaderInner { builder, body }),
            body_size,
        }
    }

    pub fn try_extract_response(
        &mut self,
        read_buffer: &mut TcpBuffer,
    ) -> Result<http::Response<BoxBody<Bytes, String>>, HttpParseError> {
        let inner = self.inner.take();

        if inner.is_none() {
            panic!("Somehow we do not have body");
        }

        let mut inner = inner.unwrap();

        if inner.body.len() == self.body_size {
            let response = inner.into_body(false);
            return Ok(response);
        }

        let remain_to_read = self.body_size - inner.body.len();

        let content = read_buffer.get_as_much_as_possible(remain_to_read);

        let content = match content {
            Ok(result) => result,
            Err(err) => {
                self.inner = Some(inner);
                return Err(err);
            }
        };

        inner.body.extend_from_slice(content);

        if inner.body.len() == self.body_size {
            let response = inner.into_body(false);
            return Ok(response);
        }

        self.inner = Some(inner);
        Err(HttpParseError::GetMoreData)
    }
}
