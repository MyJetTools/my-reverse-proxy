use std::time::Duration;

use crate::{
    h1_proxy_server::H1Writer, network_stream::*, tcp_utils::LoopBuffer, types::HttpTimeouts,
};

/// A source fed from a fixed byte vector, handing out at most `max_read` bytes per
/// read and counting the reads. Once it is exhausted a read returns 0, which
/// `read_with_timeout` surfaces as `NetworkError::Disconnected` — so a pump that
/// reads when it should not fails the test instead of hanging on a timeout.
pub struct FakeSource {
    data: Vec<u8>,
    pos: usize,
    max_read: usize,
    pub reads: usize,
}

impl FakeSource {
    /// A source that has nothing more to give — every read is a defect.
    pub fn exhausted() -> Self {
        Self::with_max_read(Vec::new(), usize::MAX)
    }

    pub fn with_max_read(data: Vec<u8>, max_read: usize) -> Self {
        Self {
            data,
            pos: 0,
            max_read,
            reads: 0,
        }
    }
}

#[async_trait::async_trait]
impl NetworkStreamReadPart for FakeSource {
    async fn read_from_socket(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.reads += 1;
        let to_read = (self.data.len() - self.pos)
            .min(buf.len())
            .min(self.max_read);
        buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }
}

/// Records every relay step, so a test can assert both the bytes and how they were
/// split across writes.
pub struct RelaySink {
    pub writes: Vec<Vec<u8>>,
}

impl RelaySink {
    pub fn new() -> Self {
        Self { writes: Vec::new() }
    }

    pub fn written(&self) -> Vec<u8> {
        self.writes.concat()
    }

    pub fn write_sizes(&self) -> Vec<usize> {
        self.writes.iter().map(|w| w.len()).collect()
    }
}

#[async_trait::async_trait]
impl H1Writer for RelaySink {
    async fn write_http_payload(
        &mut self,
        _request_id: u64,
        buffer: &[u8],
        _timeout: Duration,
    ) -> Result<(), NetworkError> {
        self.writes.push(buffer.to_vec());
        Ok(())
    }
}

/// Put bytes into the loop buffer as if they had already been read off the socket
/// (a fast peer lands the head and a large part of the body in one read).
pub fn preloaded_buffer(data: &[u8]) -> LoopBuffer {
    let mut loop_buffer = LoopBuffer::new();
    let buffer = loop_buffer.get_mut().unwrap();
    buffer[..data.len()].copy_from_slice(data);
    loop_buffer.advance(data.len());
    loop_buffer
}

/// Short timeouts: a pump that wrongly blocks on a read fails fast.
pub fn test_timeouts() -> HttpTimeouts {
    HttpTimeouts {
        read_timeout: Duration::from_millis(200),
        write_timeout: Duration::from_millis(200),
    }
}
