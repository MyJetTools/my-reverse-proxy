use std::time::Duration;

use tokio::sync::mpsc;

use crate::network_stream::*;

use super::super::H1Writer;

/// One unit of a single response, produced by a worker task and consumed by the
/// client-writer task. A response is a stream of `Chunk`s terminated by exactly
/// one `Done` (clean) or `Abort` (the response was truncated and the client
/// connection must be closed).
///
/// Ordering is NOT encoded here — it comes from the FIFO of per-slot receivers
/// the writer drains (see [`ResponseSlot`]); within one slot the events are
/// naturally ordered by the channel.
pub enum ResponseEvent {
    /// Raw bytes to forward to the client verbatim — the compiled response head
    /// or a body chunk (chunked-framing bytes included).
    Chunk(Vec<u8>),
    /// The response finished cleanly; the writer advances to the next slot.
    Done,
    /// The upstream broke after the writer had already begun sending this
    /// response (head / partial body), so no error page can be substituted —
    /// the only correct H1 behaviour is to close the client connection.
    Abort,
}

/// A reserved place in the client-writer's ordered output queue. The reader
/// pushes one of these onto the writer's queue at request-accept time (this is
/// what enforces response order — FIFO of slots); the worker for that request
/// owns the matching `mpsc::Sender<ResponseEvent>` and streams events in.
pub struct ResponseSlot {
    pub events: mpsc::Receiver<ResponseEvent>,
    /// Client-side write timeout for this response (endpoint-scoped, known once
    /// the route is resolved).
    pub write_timeout: Duration,
}

/// Bounded capacity of a single response's event channel. Bounds memory for a
/// non-head (pipelined-behind) response: a worker that outruns the writer
/// blocks on `send`, which back-pressures its upstream read. Picked small —
/// each `Chunk` is already a buffer-sized slice.
pub const RESPONSE_CHANNEL_CAPACITY: usize = 4;

/// An [`H1Writer`] sink that funnels response bytes into a [`ResponseEvent`]
/// channel. Lets the worker reuse `H1Reader::transfer_body` (and the
/// `transfer_known_size` / `transfer_chunked_body` primitives) to pump the
/// upstream response body straight into the writer's per-slot channel, with one
/// copy out of the reused loop buffer.
pub struct ChannelSink {
    tx: mpsc::Sender<ResponseEvent>,
}

impl ChannelSink {
    pub fn new(tx: mpsc::Sender<ResponseEvent>) -> Self {
        Self { tx }
    }
}

#[async_trait::async_trait]
impl H1Writer for ChannelSink {
    async fn write_http_payload(
        &mut self,
        _request_id: u64,
        buffer: &[u8],
        _timeout: Duration,
    ) -> Result<(), NetworkError> {
        // The receiver (writer task) going away means the client connection is
        // gone — surface it as a disconnect so the body pump stops.
        self.tx
            .send(ResponseEvent::Chunk(buffer.to_vec()))
            .await
            .map_err(|_| NetworkError::Disconnected)
    }
}

/// An [`H1Writer`] sink that streams REQUEST body chunks from the reader to the
/// worker over an `mpsc<Vec<u8>>`. Lets the reader reuse `H1Reader::transfer_body`
/// to pump the request body into the worker. A closed receiver (worker gone /
/// abandoned the request) surfaces as a disconnect so the reader stops and
/// closes the connection.
pub struct BodyChannelSink {
    tx: mpsc::Sender<Vec<u8>>,
}

impl BodyChannelSink {
    pub fn new(tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self { tx }
    }
}

#[async_trait::async_trait]
impl H1Writer for BodyChannelSink {
    async fn write_http_payload(
        &mut self,
        _request_id: u64,
        buffer: &[u8],
        _timeout: Duration,
    ) -> Result<(), NetworkError> {
        self.tx
            .send(buffer.to_vec())
            .await
            .map_err(|_| NetworkError::Disconnected)
    }
}

/// Bounded capacity of the request-body channel (reader → worker). Same
/// back-pressure rationale as [`RESPONSE_CHANNEL_CAPACITY`].
pub const REQUEST_BODY_CHANNEL_CAPACITY: usize = 4;

/// An [`H1Writer`] that discards everything. Used to DRAIN the request body off
/// the client stream for responses synthesized without an upstream (static /
/// local files), so the connection stays byte-synced for the next request.
pub struct NullSink;

#[async_trait::async_trait]
impl H1Writer for NullSink {
    async fn write_http_payload(
        &mut self,
        _request_id: u64,
        _buffer: &[u8],
        _timeout: Duration,
    ) -> Result<(), NetworkError> {
        Ok(())
    }
}
