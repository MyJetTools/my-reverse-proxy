use std::sync::Arc;

use crate::configurations::{HttpEndpointInfo, ProxyPassToConfig};
use crate::h1_remote_connection::{OwnedUpstream, Upstream, UpstreamInner};
use crate::network_stream::*;
use crate::tcp_utils::{copy_streams, LoopBuffer, WsDirection, WsTrafficRecorder};

use super::super::{H1HeadersKind, H1Reader, H1Writer, HttpConnectionInfo};

/// Everything needed to upgrade one request into a bidirectional tunnel. Built
/// by the reader when the request head asked for an upgrade; carried to the
/// connection entry, which reunites it with the reclaimed client halves.
pub struct UpgradeContext {
    pub proxy_pass_to: ProxyPassToConfig,
    pub end_point_info: Arc<HttpEndpointInfo>,
    pub http_connection_info: HttpConnectionInfo,
    pub location_id: i64,
    /// Compiled request head (with the Upgrade headers).
    pub head: Vec<u8>,
    pub write_timeout: std::time::Duration,
}

/// Establish a websocket/CONNECT tunnel. Connects a FRESH upstream (upgrades are
/// never pooled), forwards the request head, and on a confirmed upgrade response
/// wires the four halves into two `copy_streams` pumps. The client read half +
/// its leftover come from the reader; the client write half is reclaimed from
/// the writer task; the upstream halves come from the fresh connection.
pub async fn run_ws_tunnel<
    ClientRead: NetworkStreamReadPart + Send + 'static,
    ClientWrite: NetworkStreamWritePart + Send + Sync + 'static,
>(
    ctx: UpgradeContext,
    client_read: ClientRead,
    client_leftover: LoopBuffer,
    mut client_write: ClientWrite,
) {
    let timeouts = ctx.end_point_info.timeouts;

    let owned = match Upstream::connect_owned(&ctx.proxy_pass_to).await {
        Ok(o) => o,
        Err(_) => {
            client_write.shutdown_socket().await;
            return;
        }
    };
    let OwnedUpstream {
        mut upstream,
        response_read,
        ssh_handler,
        ..
    } = owned;

    if !upstream.send_head_bytes(&ctx.head, ctx.write_timeout).await {
        client_write.shutdown_socket().await;
        return;
    }

    let mut resp_reader = H1Reader::new(response_read, timeouts);

    let resp_headers = match resp_reader.read_headers().await {
        Ok(h) => h,
        Err(_) => {
            client_write.shutdown_socket().await;
            return;
        }
    };

    // Capture the response framing before compile_headers consumes the headers —
    // needed to relay the body on a refused upgrade.
    let response_content_length = resp_headers.content_length;

    let response_is_upgrade = match resp_reader.compile_headers(
        resp_headers,
        H1HeadersKind::Response(&ctx.end_point_info),
        &ctx.http_connection_info,
        &None,
        None,
        None,
    ) {
        Ok(ws) => ws,
        Err(_) => {
            client_write.shutdown_socket().await;
            return;
        }
    };

    // Relay the response head (the 101, or whatever the upstream answered).
    if client_write
        .write_all_with_timeout(resp_reader.h1_headers_builder.as_slice(), ctx.write_timeout)
        .await
        .is_err()
    {
        client_write.shutdown_socket().await;
        return;
    }

    if !response_is_upgrade {
        // Upgrade refused: this is a normal HTTP response, not a tunnel. Relay the
        // body to the client (mirroring the request path) before closing, so the
        // client gets a complete response instead of head + abrupt FIN.
        let mut sink = ClientWriteSink { write: client_write };
        let _ = resp_reader
            .transfer_body(0, &mut sink, response_content_length)
            .await;
        sink.write.shutdown_socket().await;
        return;
    }

    let (upstream_read, upstream_leftover) = resp_reader.into_read_part();

    let ws_domain = ctx.end_point_info.host_endpoint.as_str().to_string();
    let log_scope = crate::app::ProxyLogScope::new(
        Arc::new(ws_domain.clone()),
        ctx.location_id,
        ctx.http_connection_info.connection_ip.get_ip_log(),
    );

    let s2c_recorder = Some(WsTrafficRecorder {
        domain: ws_domain.clone(),
        direction: WsDirection::ServerToClient,
    });
    let c2s_recorder = Some(WsTrafficRecorder {
        domain: ws_domain,
        direction: WsDirection::ClientToServer,
    });

    // Server -> Client: upstream read half (type-erased) into the client write half.
    crate::app::spawn_named(
        "h1_ws_pump_server_to_client",
        copy_streams(
            upstream_read,
            client_write,
            upstream_leftover,
            ssh_handler,
            s2c_recorder,
            Some(log_scope.clone()),
            timeouts,
        ),
    );

    // Client -> Server: client read half into the upstream write half. The write
    // half type differs per transport, so extract it per variant.
    macro_rules! pump_c2s {
        ($write_half:expr) => {
            crate::app::spawn_named(
                "h1_ws_pump_client_to_server",
                copy_streams(
                    client_read,
                    $write_half,
                    client_leftover,
                    None,
                    c2s_recorder,
                    Some(log_scope),
                    timeouts,
                ),
            )
        };
    }

    match upstream.inner {
        UpstreamInner::Http1Direct(inner) => {
            pump_c2s!(inner.remote_write_part);
        }
        UpstreamInner::Http1UnixSocket(inner) => {
            pump_c2s!(inner.remote_write_part);
        }
        UpstreamInner::Https1Direct(inner) => {
            pump_c2s!(inner.remote_write_part);
        }
        UpstreamInner::Http1OverSsh(inner) => {
            pump_c2s!(inner.remote_write_part);
        }
        UpstreamInner::Http1OverGateway(inner) => {
            pump_c2s!(inner.remote_write_part);
        }
    };
}

/// An [`H1Writer`] that owns a client write half — lets a refused-upgrade
/// response body be relayed to the client via `H1Reader::transfer_body`.
struct ClientWriteSink<W: NetworkStreamWritePart + Send + Sync + 'static> {
    write: W,
}

#[async_trait::async_trait]
impl<W: NetworkStreamWritePart + Send + Sync + 'static> H1Writer for ClientWriteSink<W> {
    async fn write_http_payload(
        &mut self,
        _request_id: u64,
        buffer: &[u8],
        timeout: std::time::Duration,
    ) -> Result<(), NetworkError> {
        self.write.write_all_with_timeout(buffer, timeout).await
    }
}
