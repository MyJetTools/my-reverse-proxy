use std::sync::Arc;

use tokio::sync::mpsc;

use crate::configurations::{HttpEndpointInfo, ProxyPassToConfig};
use crate::h1_proxy_server::{
    H1HeadersKind, H1Reader, H1Writer, HttpConnectionInfo, ProxyServerError,
};
use crate::h1_remote_connection::{H1PoolHolder, McpUpstream, OwnedUpstream, Upstream};
use crate::h1_utils::HttpContentLength;
use crate::network_stream::NetworkError;

use super::{ChannelSink, ResponseEvent};

/// Total attempts to deliver the request to an upstream. The request is replayed
/// (head + buffered body) on a fresh connection only while NOTHING has reached
/// the client yet — see [`run_upstream_request`] for the exact rule.
const MAX_DELIVERY_ATTEMPTS: u32 = 2;

/// Everything one request's worker needs. Built by the reader and handed to a
/// spawned worker task.
pub struct UpstreamRequest {
    pub pool: Arc<H1PoolHolder>,
    /// Set exactly for `mcp` locations: the single connection this client TCP
    /// keeps, used INSTEAD of `pool`. Its presence is what makes this an MCP
    /// request — there is no `is_mcp` flag to disagree with it.
    pub mcp: Option<Arc<McpUpstream>>,
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

/// Drive one request against an upstream: acquire a connection, write the head +
/// body, read the response, and stream it back as [`ResponseEvent`]s.
///
/// Replay rule — the dividing line is whether the CLIENT has seen anything yet,
/// not which side of the exchange failed:
/// - While no response byte has been handed to the client, a failure on an
///   already-established (reused) connection is treated as that connection being
///   stale: reset it and replay head + buffered body on a fresh one, up to
///   [`MAX_DELIVERY_ATTEMPTS`]. The replay is invisible, so it is safe even for a
///   non-idempotent POST.
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
        mcp,
        proxy_pass_to,
        end_point_info,
        http_connection_info,
        location_id,
        head,
        mut body_rx,
        response_tx,
    } = req;

    let endpoint = end_point_info.host_endpoint.as_str();
    let ip = http_connection_info.connection_ip.get_ip_log();
    // mcp keeps its one connection on the client TCP instead of in the pool.
    let is_mcp = mcp.is_some();

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

    // Buffer the request body up front so head+body can be replayed on a fresh
    // connection if a write to the upstream fails. (Byte-path bodies are small —
    // API / MCP JSON.)
    let mut body = Vec::new();
    while let Some(chunk) = body_rx.recv().await {
        body.extend_from_slice(&chunk);
    }
    let bytes_to_upstream = body.len() as u64;

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
        let last_attempt = attempt >= MAX_DELIVERY_ATTEMPTS;

        let (mut owned, reused) = match acquire(mcp.as_ref(), &pool, &proxy_pass_to, attempt).await
        {
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
                )
                .await;
                return; // dropping body_rx unblocks the reader
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
            if !last_attempt {
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
            )
            .await;
            return;
        }

        // Write the (buffered) body. Same rule: a write failure means it did not
        // get through → reset and retry.
        if !body.is_empty()
            && owned
                .upstream
                .write_http_payload(0, &body, timeouts.write_timeout)
                .await
                .is_err()
        {
            drop(owned);
            if !last_attempt {
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
                if reused && !last_attempt && is_stale_connection(&err) {
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
                fail_request(&response_tx, is_mcp, page).await;
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
                fail_request(&response_tx, is_mcp, page).await;
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

    crate::app::APP_CTX
        .traffic
        .record_c2s(endpoint, bytes_to_upstream);

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
        match mcp.as_ref() {
            Some(mcp) => mcp.put(owned),
            None => pool.release(&proxy_pass_to, owned),
        }
    }
}

/// Check out a connection for one delivery attempt, reporting whether it was
/// already established (`true`) or freshly dialled (`false`) — the replay rule
/// turns on that distinction.
///
/// mcp bypasses the pool entirely: its one connection lives on the client TCP.
/// Only the first attempt may reuse it; a retry exists precisely because the
/// kept one was no good, so it always dials.
async fn acquire(
    mcp: Option<&Arc<McpUpstream>>,
    pool: &H1PoolHolder,
    proxy_pass_to: &ProxyPassToConfig,
    attempt: u32,
) -> Result<(OwnedUpstream, bool), NetworkError> {
    let Some(mcp) = mcp else {
        return pool.acquire(proxy_pass_to).await;
    };

    if attempt == 1 {
        if let Some(owned) = mcp.take() {
            return Ok((owned, true));
        }
    }

    Ok((Upstream::connect_owned(proxy_pass_to).await?, false))
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
/// Best-effort: if the writer/client is gone the sends just fail.
async fn fail_request(response_tx: &mpsc::Sender<ResponseEvent>, is_mcp: bool, page: &[u8]) {
    if is_mcp {
        let _ = response_tx.send(ResponseEvent::Abort).await;
        return;
    }

    if response_tx
        .send(ResponseEvent::Chunk(page.to_vec()))
        .await
        .is_ok()
    {
        let _ = response_tx.send(ResponseEvent::Done).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
        fail_request(&tx, true, b"<html>bad gateway</html>").await;
        assert!(matches!(rx.recv().await, Some(ResponseEvent::Abort)));
        drop(tx);
        assert!(rx.recv().await.is_none());
    }

    /// Everything else still gets the page, followed by a clean end of response.
    #[tokio::test]
    async fn a_plain_location_is_failed_with_the_error_page() {
        let page = b"<html>bad gateway</html>";
        let (tx, mut rx) = mpsc::channel(4);
        fail_request(&tx, false, page).await;
        match rx.recv().await {
            Some(ResponseEvent::Chunk(bytes)) => assert_eq!(bytes, page.to_vec()),
            Some(_) => panic!("expected the page chunk, got another event"),
            None => panic!("expected the page chunk, got nothing"),
        }
        assert!(matches!(rx.recv().await, Some(ResponseEvent::Done)));
    }
}
