use std::time::Duration;

use crate::network_stream::*;

use super::super::H1Writer;

/// An [`H1Writer`] that collects a request body in memory, up to a limit.
///
/// The OAuth token and consent endpoints take a form-urlencoded body of a few
/// hundred bytes, so the cap is what stops a request claiming a gigabyte from
/// being believed. Bytes past the limit are dropped rather than the transfer
/// being aborted: the body still has to be consumed in full or the connection
/// desyncs and every request after it on the same socket is garbage.
pub struct BodyCollectorSink {
    body: Vec<u8>,
    limit: usize,
    overflowed: bool,
}

impl BodyCollectorSink {
    pub fn new(limit: usize) -> Self {
        Self {
            body: Vec::new(),
            limit,
            overflowed: false,
        }
    }

    /// The collected body, or `None` when it was larger than the limit.
    pub fn into_body(self) -> Option<Vec<u8>> {
        if self.overflowed {
            return None;
        }

        Some(self.body)
    }
}

#[async_trait::async_trait]
impl H1Writer for BodyCollectorSink {
    async fn write_http_payload(
        &mut self,
        _request_id: u64,
        buffer: &[u8],
        _timeout: Duration,
    ) -> Result<(), NetworkError> {
        if self.overflowed {
            return Ok(());
        }

        if self.body.len() + buffer.len() > self.limit {
            self.overflowed = true;
            self.body = Vec::new();
            return Ok(());
        }

        self.body.extend_from_slice(buffer);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn write(sink: &mut BodyCollectorSink, payload: &[u8]) {
        sink.write_http_payload(0, payload, Duration::from_secs(1))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn chunks_are_joined_in_order() {
        let mut sink = BodyCollectorSink::new(64);

        write(&mut sink, b"grant_type=").await;
        write(&mut sink, b"refresh_token").await;

        assert_eq!(sink.into_body().unwrap(), b"grant_type=refresh_token");
    }

    #[tokio::test]
    async fn a_body_over_the_limit_is_dropped_but_still_accepted() {
        let mut sink = BodyCollectorSink::new(8);

        write(&mut sink, b"0123456789").await;
        // Later chunks keep being accepted, so the caller can finish draining
        // the request and reuse the connection.
        write(&mut sink, b"more").await;

        assert!(sink.into_body().is_none());
    }

    #[tokio::test]
    async fn an_empty_body_collects_to_nothing() {
        let sink = BodyCollectorSink::new(64);

        assert_eq!(sink.into_body().unwrap(), Vec::<u8>::new());
    }
}
