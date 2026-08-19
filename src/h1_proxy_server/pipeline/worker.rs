use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::configurations::{HttpEndpointInfo, ProxyPassToConfig};
use crate::h1_proxy_server::{
    H1HeadersKind, H1Reader, H1Writer, HttpConnectionInfo, ProxyServerError,
};
use crate::h1_remote_connection::{H1PoolHolder, OwnedUpstream};
use crate::h1_utils::HttpContentLength;

use super::{ChannelSink, ResponseEvent};

/// Total attempts to deliver the request to an upstream. The request is replayed
/// (head + the part of the body already taken from the client) on a fresh
/// connection only while NOTHING has reached the client yet AND that part still
/// fits [`REPLAY_WINDOW`] — see [`run_upstream_request`] for the exact rule.
const MAX_DELIVERY_ATTEMPTS: u32 = 2;

/// How much of an already-forwarded request body is kept in memory so the request
/// can be replayed on a fresh connection.
///
/// The body itself is STREAMED to the upstream as the reader delivers it — this
/// only bounds the replay window. Inside it a stale kept-alive connection stays
/// invisible to the client (the case [`MAX_DELIVERY_ATTEMPTS`] exists for); past
/// it the request simply stops being replayable instead of being accumulated, so
/// an upload of any size flows through with a bounded footprint.
const REPLAY_WINDOW: usize = 64 * 1024;

/// Everything one request's worker needs. Built by the reader and handed to a
/// spawned worker task.
pub struct UpstreamRequest {
    pub pool: Arc<H1PoolHolder>,
    /// Identity of the upstream (also the pool key). Owned because for
    /// dynamic_proxy it is synthesized per request.
    pub proxy_pass_to: ProxyPassToConfig,
    pub end_point_info: Arc<HttpEndpointInfo>,
    pub http_connection_info: HttpConnectionInfo,
    pub location_id: i64,
    /// Compiled request head (status line + headers), ready to write to upstream.
    pub head: Vec<u8>,
    /// Request body chunks streamed from the reader; closed (sender dropped)
    /// when the body is fully forwarded.
    pub body_rx: mpsc::Receiver<Vec<u8>>,
    /// Response events to the client-writer for this request's slot.
    pub response_tx: mpsc::Sender<ResponseEvent>,
}

/// Pipes a request body from the reader to the upstream as it arrives, keeping
/// the first [`REPLAY_WINDOW`] bytes so one delivery attempt can be repeated on a
/// fresh connection.
struct RequestBodyPump {
    rx: mpsc::Receiver<Vec<u8>>,
    /// Everything taken from `rx` so far, or `None` once the body outgrew the
    /// window — from that point the request cannot be replayed.
    replay: Option<Vec<u8>>,
    /// The reader closed its side: the whole body has been taken off the client.
    completed: bool,
    /// Body bytes taken from the client (traffic accounting). Counted once, so a
    /// replayed request is not counted twice.
    total: u64,
}

impl RequestBodyPump {
    fn new(rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            replay: Some(Vec::new()),
            completed: false,
            total: 0,
        }
    }

    /// Whether everything taken from the client so far can still be re-sent.
    fn is_replayable(&self) -> bool {
        self.replay.is_some()
    }

    /// Re-send the part of the body already taken from the client onto a fresh
    /// connection. A no-op on the first attempt (nothing taken yet).
    async fn resend<TWriter: H1Writer + Send + Sync + 'static>(
        &self,
        upstream: &mut TWriter,
        write_timeout: Duration,
    ) -> bool {
        let Some(replay) = self.replay.as_ref() else {
            return false; // must not be called for a non-replayable request
        };

        if replay.is_empty() {
            return true;
        }

        upstream
            .write_http_payload(0, replay, write_timeout)
            .await
            .is_ok()
    }

    /// Forward the rest of the body chunk by chunk, as the reader delivers it.
    /// Nothing is accumulated beyond the replay window.
    async fn stream<TWriter: H1Writer + Send + Sync + 'static>(
        &mut self,
        upstream: &mut TWriter,
        write_timeout: Duration,
    ) -> bool {
        while let Some(chunk) = self.rx.recv().await {
            self.total += chunk.len() as u64;

            // Remembered BEFORE the write: a chunk that fails on the wire still
            // has to be part of a replay.
            if let Some(replay) = self.replay.as_mut() {
                if replay.len() + chunk.len() > REPLAY_WINDOW {
                    self.replay = None;
                } else {
                    replay.extend_from_slice(&chunk);
                }
            }

            if upstream
                .write_http_payload(0, &chunk, write_timeout)
                .await
                .is_err()
            {
                return false;
            }
        }

        self.completed = true;
        true
    }

    /// Take (and discard) whatever is left of the body after the request has
    /// already failed, so the CLIENT connection stays byte-synced for the next
    /// request. Bounded by the replay window: reading an arbitrarily large upload
    /// only to throw it away is not worth it — `false` means the connection has to
    /// be closed instead.
    async fn discard_rest(&mut self) -> bool {
        if self.completed {
            return true;
        }

        let mut discarded = 0usize;

        while let Some(chunk) = self.rx.recv().await {
            self.total += chunk.len() as u64;
            discarded += chunk.len();
            if discarded > REPLAY_WINDOW {
                return false;
            }
        }

        self.completed = true;
        true
    }
}

/// Drive one request against an upstream: acquire a connection, write the head,
/// stream the body, read the response, and stream it back as [`ResponseEvent`]s.
///
/// Replay rule — the dividing line is whether the CLIENT has seen anything yet,
/// not which side of the exchange failed:
/// - While no response byte has been handed to the client, a failure on an
///   already-established (reused) connection is treated as that connection being
///   stale: reset it and replay head + the already-forwarded body on a fresh one,
///   up to [`MAX_DELIVERY_ATTEMPTS`]. The replay is invisible, so it is safe even
///   for a non-idempotent POST. It needs the body still to be inside
///   [`REPLAY_WINDOW`]; a bigger body is streamed through and answered instead of
///   being replayed.
///   This covers the case a write-only rule cannot see: when the upstream closed
///   a kept connection, the request write still SUCCEEDS (the bytes land in the
///   kernel send buffer) and only the response read reports the loss.
/// - A failure on a FRESHLY dialled connection is a broken upstream, not
///   staleness — replaying it would just hit the same wall, so it is answered.
/// - A timeout is never replayed: the upstream may be working on the request.
/// - Once the client has received bytes nothing can be substituted or replayed —
///   the response is aborted and the client connection closed.
pub async fn run_upstream_request(req: UpstreamRequest) {
    let UpstreamRequest {
        pool,
        proxy_pass_to,
        end_point_info,
        http_connection_info,
        location_id,
        head,
        body_rx,
        response_tx,
    } = req;

    let endpoint = end_point_info.host_endpoint.as_str();
    let ip = http_connection_info.connection_ip.get_ip_log();
    let is_mcp = proxy_pass_to.is_mcp();

    // An MCP listening stream is SSE that legitimately idles with no keepalive
    // between server-initiated messages, far longer than a normal response body
    // ever would. Reading it on the endpoint's ordinary read timeout tears down
    // a perfectly healthy stream every few minutes, which the client can only
    // see as the transport dropping. The hyper path already carves this out
    // (`DEFAULT_MCP_READ_TIMEOUT` via `PoolParams::read_stream_timeout`); this
    // path reads the upstream itself, so it needs the same carve-out.
    let mut timeouts = end_point_info.timeouts;
    if is_mcp {
        timeouts.read_timeout = crate::consts::DEFAULT_MCP_READ_TIMEOUT;
    }

    // The body is piped to the upstream as the reader delivers it — the worker
    // never waits for (nor holds) the whole thing; only the first REPLAY_WINDOW
    // bytes are remembered, for a possible replay.
    let mut body = RequestBodyPump::new(body_rx);

    // Deliver the request, replaying it while the client has seen nothing yet.
    // Breaks with a connection whose response head has been read.
    let mut attempt = 0u32;
    let (
        upstream,
        mut resp_reader,
        response_content_length,
        response_is_websocket,
        disconnect_trigger,
        ssh_handler,
    ) = loop {
        attempt += 1;
        // Whether a replay is still possible is re-checked at every failure site:
        // streaming the body can end the replay window mid-attempt.
        let retries_left = attempt < MAX_DELIVERY_ATTEMPTS;

        let (mut owned, reused) = match pool.acquire(&proxy_pass_to).await {
            Ok(c) => c,
            Err(err) => {
                crate::app::APP_CTX.proxy_logs.write_returned_5xx(
                    endpoint,
                    Some(location_id),
                    ip.clone(),
                    503,
                    format!(
                        "can not connect to upstream {}: {:?}",
                        proxy_pass_to.to_string(),
                        err
                    ),
                );
                fail_request(
                    &response_tx,
                    is_mcp,
                    crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice(),
                    &mut body,
                )
                .await;
                return;
            }
        };

        // Write the head. Failure = the request did not reach a working upstream
        // → reset the connection and retry on a fresh one.
        if !owned
            .upstream
            .send_head_bytes(&head, timeouts.write_timeout)
            .await
        {
            drop(owned); // reset (the dead connection is never pooled)
            if retries_left && body.is_replayable() {
                continue;
            }
            crate::app::APP_CTX.proxy_logs.write_returned_5xx(
                endpoint,
                Some(location_id),
                ip.clone(),
                503,
                format!(
                    "upstream {} did not accept request head",
                    proxy_pass_to.to_string()
                ),
            );
            fail_request(
                &response_tx,
                is_mcp,
                crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice(),
                &mut body,
            )
            .await;
            return;
        }

        // Re-deliver the part of the body already taken from the client (nothing
        // on the first attempt), then stream the rest through as it arrives. Same
        // rule as the head: a write failure means it did not get through → reset
        // and retry.
        let delivered = body
            .resend(&mut owned.upstream, timeouts.write_timeout)
            .await
            && body
                .stream(&mut owned.upstream, timeouts.write_timeout)
                .await;

        if !delivered {
            drop(owned);
            if retries_left && body.is_replayable() {
                continue;
            }
            crate::app::APP_CTX.proxy_logs.write_returned_5xx(
                endpoint,
                Some(location_id),
                ip.clone(),
                502,
                format!(
                    "upstream {} broke while forwarding request body",
                    proxy_pass_to.to_string()
                ),
            );
            fail_request(
                &response_tx,
                is_mcp,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                &mut body,
            )
            .await;
            return;
        }

        // Read the response head. On a REUSED connection a transport failure here
        // means the upstream had closed it while it was parked — the write could
        // not tell us, because it only reached the kernel send buffer. Nothing
        // has been handed to the client, so replay on a fresh connection.
        let OwnedUpstream {
            upstream,
            response_read,
            disconnect_trigger,
            ssh_handler,
        } = owned;
        let mut resp_reader = H1Reader::new(response_read, timeouts);

        let resp_headers = match resp_reader.read_headers().await {
            Ok(h) => h,
            Err(err) => {
                disconnect_trigger.set_value(true);
                if reused && retries_left && body.is_replayable() && is_stale_connection(&err) {
                    continue;
                }
                let (page, label) = classify_upstream_failure(&err);
                crate::app::APP_CTX.proxy_logs.write_returned_5xx(
                    endpoint,
                    Some(location_id),
                    ip.clone(),
                    502,
                    format!(
                        "reading response head from upstream {} ({}): {:?}",
                        proxy_pass_to.to_string(),
                        label,
                        err
                    ),
                );
                fail_request(&response_tx, is_mcp, page, &mut body).await;
                return;
            }
        };

        let response_content_length = resp_headers.content_length;

        let response_is_websocket = match resp_reader.compile_headers(
            resp_headers,
            H1HeadersKind::Response(&end_point_info),
            &http_connection_info,
            &None,
            None,
            None,
        ) {
            Ok(ws) => ws,
            Err(err) => {
                disconnect_trigger.set_value(true);
                let (page, label) = classify_upstream_failure(&err);
                crate::app::APP_CTX.proxy_logs.write_returned_5xx(
                    endpoint,
                    Some(location_id),
                    ip.clone(),
                    502,
                    format!(
                        "compiling response head from upstream {} ({}): {:?}",
                        proxy_pass_to.to_string(),
                        label,
                        err
                    ),
                );
                fail_request(&response_tx, is_mcp, page, &mut body).await;
                return;
            }
        };

        break (
            upstream,
            resp_reader,
            response_content_length,
            response_is_websocket,
            disconnect_trigger,
            ssh_handler,
        );
    };

    crate::app::APP_CTX.traffic.record_c2s(endpoint, body.total);

    // From here on the client begins receiving the response — any failure must
    // Abort (close the connection), never substitute an error page.
    if response_tx
        .send(ResponseEvent::Chunk(
            resp_reader.h1_headers_builder.as_slice().to_vec(),
        ))
        .await
        .is_err()
    {
        return; // writer/client gone
    }

    if response_is_websocket {
        // A non-upgrade request whose response unexpectedly upgrades cannot be
        // tunneled on this path — close the connection.
        let _ = response_tx.send(ResponseEvent::Abort).await;
        return;
    }

    let mut sink = ChannelSink::new(response_tx.clone());
    let bytes_to_client = match resp_reader
        .transfer_body(upstream.connection_id, &mut sink, response_content_length)
        .await
    {
        Ok(bytes) => bytes,
        Err(_) => {
            // Response truncated mid-body — cannot recover, close the connection.
            let _ = response_tx.send(ResponseEvent::Abort).await;
            return;
        }
    };

    if response_tx.send(ResponseEvent::Done).await.is_err() {
        return;
    }

    crate::app::APP_CTX
        .traffic
        .record_s2c(endpoint, bytes_to_client as u64);

    // --- Keep-alive reuse. Keep only when safely reusable: self-delimiting
    // (Content-Length / chunked) response, no leftover bytes, live socket.
    // Everything else is dropped (closed). An SSE response is not
    // self-delimiting, so a streaming mcp connection is never kept — only the
    // short JSON request/response calls are. ---
    let reusable_framing = matches!(
        response_content_length,
        HttpContentLength::Known(_) | HttpContentLength::Chunked
    );
    let (response_read, leftover) = resp_reader.into_read_part();
    if reusable_framing && leftover.get_data().is_empty() && !disconnect_trigger.get_value() {
        let owned = OwnedUpstream {
            upstream,
            response_read,
            disconnect_trigger,
            ssh_handler,
        };
        pool.release(&proxy_pass_to, owned);
    }
}

/// Whether an upstream failure looks like a connection that went stale rather
/// than an upstream that is broken — i.e. the transport dropped. A timeout does
/// NOT qualify: the upstream may be processing the request, so replaying it
/// could duplicate a side effect. Neither does a parse error — bytes came back,
/// the connection was alive, the upstream just is not speaking HTTP.
fn is_stale_connection(err: &ProxyServerError) -> bool {
    matches!(err, ProxyServerError::NetworkError(e) if !e.is_timeout())
}

/// Pick the client error page for an upstream-response failure: unparseable
/// bytes → "upstream is not HTTP"; a timeout → timeout; anything else
/// (disconnect / io) → "remote resource is not available".
fn classify_upstream_failure(err: &ProxyServerError) -> (&'static [u8], &'static str) {
    match err {
        ProxyServerError::NetworkError(e) if e.is_timeout() => {
            (crate::error_templates::ERROR_TIMEOUT.as_slice(), "timeout")
        }
        ProxyServerError::NetworkError(_) => (
            crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice(),
            "disconnected",
        ),
        // Got bytes but they are not valid HTTP.
        _ => (
            crate::error_templates::UPSTREAM_IS_NOT_HTTP.as_slice(),
            "non-HTTP response",
        ),
    }
}

/// Finish a request that failed before the client received a single byte.
///
/// For a browser-facing location that means substituting an error page. For mcp
/// it does not: the client speaks JSON-RPC over a single endpoint and an HTML
/// "Bad gateway" body is noise it cannot interpret — it would have to be parsed
/// as a protocol message and fail. The one thing such a client acts on is the
/// transport dropping, so the connection is closed and it redials.
///
/// Because the body is streamed, the failure can land mid-upload: the rest of it
/// still has to come off the client socket or the connection is desynced and
/// every request after it on that socket is garbage. What is left is discarded
/// when it is small enough to be worth reading, and the connection is closed
/// otherwise.
///
/// Best-effort: if the writer/client is gone the sends just fail.
async fn fail_request(
    response_tx: &mpsc::Sender<ResponseEvent>,
    is_mcp: bool,
    page: &[u8],
    body: &mut RequestBodyPump,
) {
    let stays_synced = body.discard_rest().await;

    if is_mcp {
        let _ = response_tx.send(ResponseEvent::Abort).await;
        return;
    }

    if response_tx
        .send(ResponseEvent::Chunk(page.to_vec()))
        .await
        .is_err()
    {
        return;
    }

    let _ = if stays_synced {
        response_tx.send(ResponseEvent::Done).await
    } else {
        // Part of the request body was left unread — nothing else can be served
        // on this connection.
        response_tx.send(ResponseEvent::Abort).await
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_stream::NetworkError;

    /// A body whose reader is already gone — nothing left to take.
    fn drained_body() -> RequestBodyPump {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        RequestBodyPump::new(rx)
    }

    /// A body pre-loaded with the chunks the reader would deliver.
    fn body_of(chunks: Vec<Vec<u8>>) -> RequestBodyPump {
        let (tx, rx) = mpsc::channel(chunks.len().max(1));
        for chunk in chunks {
            tx.try_send(chunk).unwrap();
        }
        drop(tx);
        RequestBodyPump::new(rx)
    }

    /// Records what reached the upstream, and can start failing after N writes.
    struct RecordingUpstream {
        writes: Vec<Vec<u8>>,
        fail_from: usize,
    }

    impl RecordingUpstream {
        fn new() -> Self {
            Self {
                writes: Vec::new(),
                fail_from: usize::MAX,
            }
        }

        fn failing_from(fail_from: usize) -> Self {
            Self {
                writes: Vec::new(),
                fail_from,
            }
        }

        fn written(&self) -> Vec<u8> {
            self.writes.concat()
        }
    }

    #[async_trait::async_trait]
    impl H1Writer for RecordingUpstream {
        async fn write_http_payload(
            &mut self,
            _request_id: u64,
            buffer: &[u8],
            _timeout: Duration,
        ) -> Result<(), NetworkError> {
            if self.writes.len() >= self.fail_from {
                return Err(NetworkError::Disconnected);
            }
            self.writes.push(buffer.to_vec());
            Ok(())
        }
    }

    const TIMEOUT: Duration = Duration::from_secs(1);

    /// Each chunk goes out as the reader delivers it — the pump does not join them
    /// into one write, and a small body stays replayable.
    #[tokio::test]
    async fn a_body_is_forwarded_chunk_by_chunk() {
        let mut body = body_of(vec![b"first".to_vec(), b"second".to_vec()]);
        let mut upstream = RecordingUpstream::new();

        assert!(body.stream(&mut upstream, TIMEOUT).await);

        assert_eq!(upstream.writes, vec![b"first".to_vec(), b"second".to_vec()]);
        assert_eq!(body.total, 11);
        assert!(body.completed);
        assert!(body.is_replayable());
    }

    /// A body larger than the replay window still streams through in full — it
    /// just stops being replayable instead of being accumulated.
    #[tokio::test]
    async fn a_body_past_the_replay_window_still_streams_but_cannot_be_replayed() {
        let big = vec![b'x'; REPLAY_WINDOW];
        let mut body = body_of(vec![big.clone(), b"tail".to_vec()]);
        let mut upstream = RecordingUpstream::new();

        assert!(body.stream(&mut upstream, TIMEOUT).await);

        assert_eq!(body.total, REPLAY_WINDOW as u64 + 4);
        assert_eq!(upstream.written().len(), REPLAY_WINDOW + 4);
        assert!(!body.is_replayable());
    }

    /// The failed chunk is part of the replay: a retry re-sends everything taken
    /// from the client, then continues with the rest of the stream.
    #[tokio::test]
    async fn a_retry_resends_everything_taken_from_the_client() {
        let mut body = body_of(vec![b"aa".to_vec(), b"bb".to_vec(), b"cc".to_vec()]);
        let mut broken = RecordingUpstream::failing_from(1);

        assert!(!body.stream(&mut broken, TIMEOUT).await);
        assert_eq!(broken.written(), b"aa".to_vec());
        assert!(body.is_replayable());

        let mut fresh = RecordingUpstream::new();
        assert!(body.resend(&mut fresh, TIMEOUT).await);
        assert!(body.stream(&mut fresh, TIMEOUT).await);

        assert_eq!(fresh.written(), b"aabbcc".to_vec());
        assert_eq!(body.total, 6);
    }

    /// The rest of a body has to come off the client socket before the connection
    /// can serve anything else; an oversized remainder is not worth reading.
    #[tokio::test]
    async fn the_rest_of_a_body_is_discarded_only_while_it_is_small() {
        let mut small = body_of(vec![b"left over".to_vec()]);
        assert!(small.discard_rest().await);
        assert!(small.completed);

        let mut huge = body_of(vec![vec![b'x'; REPLAY_WINDOW + 1]]);
        assert!(!huge.discard_rest().await);
    }

    /// The case the whole fix exists for: the upstream closed a kept connection,
    /// so the request write landed in the kernel buffer and only the response
    /// read reported it.
    #[test]
    fn a_dropped_transport_is_stale_and_replayable() {
        assert!(is_stale_connection(&ProxyServerError::NetworkError(
            NetworkError::Disconnected
        )));
        assert!(is_stale_connection(&ProxyServerError::NetworkError(
            NetworkError::IoError(std::io::Error::from(std::io::ErrorKind::ConnectionReset))
        )));
    }

    /// A timeout means the upstream may be working on the request — replaying it
    /// could duplicate a side effect.
    #[test]
    fn a_timeout_is_not_replayable() {
        assert!(!is_stale_connection(&ProxyServerError::NetworkError(
            NetworkError::Timeout(Duration::from_secs(1))
        )));
    }

    /// Bytes came back, so the connection was alive — the upstream just is not
    /// speaking HTTP. Retrying would hit the same wall.
    #[test]
    fn a_parse_failure_is_not_replayable() {
        assert!(!is_stale_connection(&ProxyServerError::HeadersParseError(
            "bad"
        )));
        assert!(!is_stale_connection(
            &ProxyServerError::BufferAllocationFail
        ));
    }

    #[test]
    fn upstream_failures_map_to_the_matching_page() {
        let (page, label) = classify_upstream_failure(&ProxyServerError::NetworkError(
            NetworkError::Timeout(Duration::from_secs(1)),
        ));
        assert_eq!(page, crate::error_templates::ERROR_TIMEOUT.as_slice());
        assert_eq!(label, "timeout");

        let (page, label) =
            classify_upstream_failure(&ProxyServerError::NetworkError(NetworkError::Disconnected));
        assert_eq!(
            page,
            crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
        );
        assert_eq!(label, "disconnected");

        let (page, label) = classify_upstream_failure(&ProxyServerError::HeadersParseError("bad"));
        assert_eq!(
            page,
            crate::error_templates::UPSTREAM_IS_NOT_HTTP.as_slice()
        );
        assert_eq!(label, "non-HTTP response");
    }

    /// mcp gets the connection closed instead of an error page — an HTML body is
    /// not something a JSON-RPC client can act on.
    #[tokio::test]
    async fn mcp_is_failed_by_closing_the_connection() {
        let (tx, mut rx) = mpsc::channel(4);
        fail_request(&tx, true, b"<html>bad gateway</html>", &mut drained_body()).await;
        assert!(matches!(rx.recv().await, Some(ResponseEvent::Abort)));
        drop(tx);
        assert!(rx.recv().await.is_none());
    }

    /// Everything else still gets the page, followed by a clean end of response.
    #[tokio::test]
    async fn a_plain_location_is_failed_with_the_error_page() {
        let page = b"<html>bad gateway</html>";
        let (tx, mut rx) = mpsc::channel(4);
        fail_request(&tx, false, page, &mut drained_body()).await;
        match rx.recv().await {
            Some(ResponseEvent::Chunk(bytes)) => assert_eq!(bytes, page.to_vec()),
            Some(_) => panic!("expected the page chunk, got another event"),
            None => panic!("expected the page chunk, got nothing"),
        }
        assert!(matches!(rx.recv().await, Some(ResponseEvent::Done)));
    }

    /// A failure mid-upload with an oversized remainder: the page is still served,
    /// then the connection is closed — it cannot be resynced for a next request.
    #[tokio::test]
    async fn a_failure_with_an_unread_body_closes_the_connection_after_the_page() {
        let mut body = body_of(vec![vec![b'x'; REPLAY_WINDOW + 1]]);
        let (tx, mut rx) = mpsc::channel(4);

        fail_request(&tx, false, b"<html>bad gateway</html>", &mut body).await;

        assert!(matches!(rx.recv().await, Some(ResponseEvent::Chunk(_))));
        assert!(matches!(rx.recv().await, Some(ResponseEvent::Abort)));
    }
}
