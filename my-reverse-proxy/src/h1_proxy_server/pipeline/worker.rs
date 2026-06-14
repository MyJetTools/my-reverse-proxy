use std::sync::Arc;

use tokio::sync::mpsc;

use crate::configurations::{HttpEndpointInfo, ProxyPassToConfig};
use crate::h1_proxy_server::{H1HeadersKind, H1Reader, H1Writer, HttpConnectionInfo, ProxyServerError};
use crate::h1_remote_connection::{H1PoolHolder, OwnedUpstream};
use crate::h1_utils::HttpContentLength;

use super::{ChannelSink, ResponseEvent};

/// Total attempts to deliver the request to an upstream. The request is replayed
/// (head + buffered body) on a fresh connection only when the WRITE to the
/// upstream failed — i.e. it did not get through. A response-side failure is
/// never replayed.
const MAX_DELIVERY_ATTEMPTS: u32 = 2;

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

/// Drive one request against an upstream: acquire a connection, write the head +
/// body, read the response, and stream it back as [`ResponseEvent`]s.
///
/// Replay rule:
/// - A failure WRITING the request to the upstream (head or body) means it did
///   not get through — reset the connection and retry on a fresh one (up to
///   [`MAX_DELIVERY_ATTEMPTS`]). The request body is buffered so it can be
///   replayed.
/// - A failure READING the response (timeout / disconnect / garbage) means the
///   upstream may have already received and processed the request — it is NEVER
///   replayed; we disconnect the socket and answer the matching 5xx.
pub async fn run_upstream_request(req: UpstreamRequest) {
    let UpstreamRequest {
        pool,
        proxy_pass_to,
        end_point_info,
        http_connection_info,
        location_id,
        head,
        mut body_rx,
        response_tx,
    } = req;

    let timeouts = end_point_info.timeouts;
    let endpoint = end_point_info.host_endpoint.as_str();
    let ip = http_connection_info.connection_ip.get_ip_log();
    // mcp upstreams are never pooled — each request gets a fresh connection
    // (their responses can be long-lived SSE streams).
    let is_mcp = matches!(proxy_pass_to, ProxyPassToConfig::McpHttp1(_));

    // Buffer the request body up front so head+body can be replayed on a fresh
    // connection if a write to the upstream fails. (Byte-path bodies are small —
    // API / MCP JSON.)
    let mut body = Vec::new();
    while let Some(chunk) = body_rx.recv().await {
        body.extend_from_slice(&chunk);
    }
    let bytes_to_upstream = body.len() as u64;

    // Deliver the request, retrying on a WRITE failure (request did not get
    // through). Breaks with a connection whose response head is ready to read.
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

        let (mut owned, _reused) = match pool.acquire(&proxy_pass_to).await {
            Ok(c) => c,
            Err(err) => {
                crate::app::APP_CTX.proxy_logs.write_returned_5xx(
                    endpoint,
                    Some(location_id),
                    ip.clone(),
                    503,
                    format!("can not connect to upstream {}: {:?}", proxy_pass_to.to_string(), err),
                );
                emit_error_page(
                    &response_tx,
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
                format!("upstream {} did not accept request head", proxy_pass_to.to_string()),
            );
            emit_error_page(
                &response_tx,
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
                format!("upstream {} broke while forwarding request body", proxy_pass_to.to_string()),
            );
            emit_error_page(
                &response_tx,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
            )
            .await;
            return;
        }

        // Read the response head. Read-side failure is NEVER replayed: mark the
        // socket dead and answer the matching 5xx (timeout/disconnect →
        // unavailable; unparseable bytes → upstream-is-not-HTTP).
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
                let (page, label) = classify_upstream_failure(&err);
                crate::app::APP_CTX.proxy_logs.write_returned_5xx(
                    endpoint,
                    Some(location_id),
                    ip.clone(),
                    502,
                    format!("reading response head from upstream {} ({}): {:?}", proxy_pass_to.to_string(), label, err),
                );
                emit_error_page(&response_tx, page).await;
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
                    format!("compiling response head from upstream {} ({}): {:?}", proxy_pass_to.to_string(), label, err),
                );
                emit_error_page(&response_tx, page).await;
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

    crate::app::APP_CTX.traffic.record_c2s(endpoint, bytes_to_upstream);

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

    // --- Keep-alive reuse. Pool only when safely reusable: self-delimiting
    // (Content-Length / chunked) response, non-mcp, no leftover bytes, live
    // socket. Everything else is dropped (closed). ---
    let reusable_framing = matches!(
        response_content_length,
        HttpContentLength::Known(_) | HttpContentLength::Chunked
    );
    let (response_read, leftover) = resp_reader.into_read_part();
    if !is_mcp
        && reusable_framing
        && leftover.get_data().is_empty()
        && !disconnect_trigger.get_value()
    {
        pool.release(
            &proxy_pass_to,
            OwnedUpstream {
                upstream,
                response_read,
                disconnect_trigger,
                ssh_handler,
            },
        );
    }
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

/// Send a ready-made error page to the client and finish the response slot.
/// Best-effort: if the writer/client is gone the sends just fail.
async fn emit_error_page(response_tx: &mpsc::Sender<ResponseEvent>, page: &[u8]) {
    if response_tx
        .send(ResponseEvent::Chunk(page.to_vec()))
        .await
        .is_ok()
    {
        let _ = response_tx.send(ResponseEvent::Done).await;
    }
}
