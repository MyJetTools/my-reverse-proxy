use bytes::Bytes;
use http_body_util::combinators::BoxBody;

use super::*;
use crate::my_http_client::{HttpParseError, TcpBuffer};

#[derive(Debug)]
pub struct FullBodyReader {
    inner: Option<FullBodyReaderInner>,
    pub body_size: usize,
}

impl FullBodyReader {
    pub fn new(builder: http::response::Builder, body_size: usize) -> Self {
        let body = Vec::with_capacity(body_size);
        Self {
            inner: Some(FullBodyReaderInner { builder, body }),
            body_size,
        }
    }

    pub fn try_extract_response(
        &mut self,
        tcp_buffer: &mut TcpBuffer,
    ) -> Result<http::Response<BoxBody<Bytes, String>>, HttpParseError> {
        let inner = self.inner.take();

        if inner.is_none() {
            panic!("Somehow we do not have body");
        }

        let mut inner = inner.unwrap();

        if inner.body.len() == self.body_size {
            let response = inner.into_body();
            return Ok(response);
        }

        let remain_to_read = self.body_size - inner.body.len();

        let content = tcp_buffer.get_as_much_as_possible(remain_to_read);

        let content = match content {
            Ok(result) => result,
            Err(err) => {
                self.inner = Some(inner);
                return Err(err);
            }
        };

        inner.body.extend_from_slice(content);

        if inner.body.len() == self.body_size {
            let response = inner.into_body();
            return Ok(response);
        }

        self.inner = Some(inner);
        Err(HttpParseError::GetMoreData)
    }
}
