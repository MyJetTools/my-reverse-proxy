use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::mpsc;

use crate::configurations::{MyReverseProxyRemoteEndpoint, ProxyPassToConfig, ProxyPassToModel};
use crate::h1_remote_connection::{mcp_path, H1PoolHolder};
use crate::network_stream::*;

use super::super::{
    H1HeadersKind, H1Reader, HttpConnectionInfo, ProxyServerError,
};
use super::{
    run_client_writer, run_upstream_request, run_ws_tunnel, BodyChannelSink, ResponseEvent,
    ResponseSlot, UpgradeContext, UpstreamRequest, REQUEST_BODY_CHANNEL_CAPACITY,
    RESPONSE_CHANNEL_CAPACITY,
};

/// How many response slots may be queued ahead of the writer. H1 serves one
/// request at a time (no pipelining), so 1 is correct: it stops the reader from
/// dispatching — and pinning upstream connections for — later requests while a
/// long/streaming (SSE) response occupies the head slot.
const RESPONSE_QUEUE_CAPACITY: usize = 1;

/// RAII per-connection metrics gauge. Uses a `parking_lot::Mutex` (not RefCell)
/// so `&ConnGauge` is `Send` — the reader holds a borrow across `.await` while
/// dispatching. The guard is never held across an await.
struct ConnGauge {
    listen_addr: String,
    port: Option<u16>,
    endpoint: Mutex<Option<String>>,
}

impl ConnGauge {
    fn attribute_to_endpoint(&self, host: &str) {
        let mut slot = self.endpoint.lock();
        if slot.is_some() {
            return;
        }
        crate::app::APP_CTX
            .metrics
            .update(|m| m.connection_by_endpoint.inc(&host.to_string()));
        *slot = Some(host.to_string());
    }
}

impl Drop for ConnGauge {
    fn drop(&mut self) {
        crate::app::APP_CTX
            .prometheus
            .dec_http1_server_connections(&self.listen_addr);
        if let Some(port) = self.port {
            crate::app::APP_CTX
                .metrics
                .update(|m| m.connection_by_port.dec(&port));
        }
        if let Some(endpoint) = self.endpoint.lock().as_ref() {
            crate::app::APP_CTX
                .metrics
                .update(|m| m.connection_by_endpoint.dec(endpoint));
        }
    }
}

/// What the per-request reader step decided about the connection.
enum ReaderStep {
    /// Keep reading the next request on this connection.
    Continue,
    /// Close the connection (error already responded, drop, or desync).
    Close,
    /// A websocket/CONNECT upgrade was requested — terminal for the connection;
    /// the connection entry reclaims the write half and runs the tunnel.
    Upgrade(UpgradeContext),
}

/// Connection entry for the task-decomposed H1 path. Spawns the client-writer
/// task (sole owner of the write half) and runs the reader loop on this task.
pub async fn serve_reverse_proxy_pipelined<
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    TServerStream: NetworkStream<WritePart = WritePart, ReadPart = ReadPart> + Send + Sync + 'static,
>(
    server_stream: TServerStream,
    mut http_connection_info: HttpConnectionInfo,
) {
    let listen_addr = http_connection_info.listen_config.listen_host.to_string();
    let port = http_connection_info.listen_config.listen_host.get_port();
    crate::app::APP_CTX
        .prometheus
        .inc_http1_server_connections(&listen_addr);
    if let Some(port) = port {
        crate::app::APP_CTX
            .metrics
            .update(|m| m.connection_by_port.inc(&port));
    }
    let conn_gauge = ConnGauge {
        listen_addr,
        port,
        endpoint: Mutex::new(None),
    };
    if let Some(endpoint_info) = http_connection_info.endpoint_info.as_ref() {
        conn_gauge.attribute_to_endpoint(endpoint_info.host_endpoint.as_str());
    }

    let (server_read_part, server_write_part) = server_stream.split();

    let timeouts = http_connection_info
        .endpoint_info
        .as_ref()
        .map(|e| e.timeouts)
        .unwrap_or_default();

    let mut h1_reader = H1Reader::new(server_read_part, timeouts);

    // Per-connection (local) pool. mcp / explicit-no-global cases use this; a
    // global pool would be passed in instead — same H1PoolHolder interface.
    let pool = H1PoolHolder::new_local();

    let (queue_tx, queue_rx) = mpsc::channel::<ResponseSlot>(RESPONSE_QUEUE_CAPACITY);
    let writer = crate::app::spawn_named(
        "h1_client_writer",
        run_client_writer(server_write_part, queue_rx),
    );

    loop {
        match read_and_dispatch(&mut h1_reader, &mut http_connection_info, &pool, &queue_tx, &conn_gauge)
            .await
        {
            ReaderStep::Continue => continue,
            ReaderStep::Close => break,
            ReaderStep::Upgrade(ctx) => {
                // Reclaim the client write half: close the queue so the writer
                // finishes any pending responses and hands the write half back.
                drop(queue_tx);
                let client_write = match writer.await {
                    Ok(Some(w)) => w,
                    // Writer closed the connection (a pending response failed) —
                    // nothing to upgrade.
                    _ => return,
                };
                let (client_read, client_leftover) = h1_reader.into_read_part();
                run_ws_tunnel(ctx, client_read, client_leftover, client_write).await;
                return;
            }
        }
    }

    // No more requests — let the writer flush the queue and close.
    drop(queue_tx);
    let _ = writer.await;
}

async fn read_and_dispatch<ReadPart: NetworkStreamReadPart + Send + Sync + 'static>(
    h1_reader: &mut H1Reader<ReadPart>,
    http_connection_info: &mut HttpConnectionInfo,
    pool: &Arc<H1PoolHolder>,
    queue_tx: &mpsc::Sender<ResponseSlot>,
    conn_gauge: &ConnGauge,
) -> ReaderStep {
    if let Some(endpoint_info) = http_connection_info.endpoint_info.as_ref() {
        conn_gauge.attribute_to_endpoint(endpoint_info.host_endpoint.as_str());
    }

    let request_headers = match h1_reader.read_headers().await {
        Ok(h) => h,
        // No bytes / transport gone — just close.
        Err(_) => return ReaderStep::Close,
    };

    let request_host_for_metric: Option<String> =
        request_headers.host_value.as_ref().and_then(|host_pos| {
            let buf = h1_reader.loop_buffer.get_data();
            if host_pos.end > buf.len() {
                return None;
            }
            let host_str = std::str::from_utf8(&buf[host_pos.start..host_pos.end]).ok()?;
            let host_no_port = match host_str.find(':') {
                Some(idx) => &host_str[..idx],
                None => host_str,
            };
            Some(host_no_port.trim().to_string())
        });

    let needs_endpoint_resolve = match http_connection_info.endpoint_info.as_ref() {
        None => true,
        Some(current) => match request_host_for_metric.as_deref() {
            Some(host) => !current.is_my_endpoint(host),
            None => false,
        },
    };

    if needs_endpoint_resolve {
        let was_none = http_connection_info.endpoint_info.is_none();
        http_connection_info.endpoint_info =
            h1_reader.try_find_endpoint_info(&request_headers, &http_connection_info.listen_config);
        if was_none && http_connection_info.endpoint_info.is_some() {
            if let Some(ip) = http_connection_info.connection_ip.get_ip_addr() {
                crate::app::APP_CTX.ip_blocklist.register_success(&ip);
            }
        }
    }

    let (location, end_point_info) =
        match h1_reader.find_location(&request_headers, http_connection_info).await {
            Ok(found) => found,
            Err(err) => {
                // LocationIsNotFound carries a resolved endpoint (so its 503 is
                // logged); HttpConfigurationIsNotFound has none (and is not 5xx).
                let ep = http_connection_info
                    .endpoint_info
                    .as_ref()
                    .map(|e| e.host_endpoint.as_str().to_string());
                return respond_error(queue_tx, http_connection_info, ep.as_deref(), err).await;
            }
        };

    h1_reader.timeouts = end_point_info.timeouts;
    let write_timeout = end_point_info.timeouts.write_timeout;
    let keep_alive = end_point_info.keep_alive;
    let endpoint_for_error = end_point_info.host_endpoint.as_str().to_string();

    if let Some(domain) = end_point_info.tracked_domain(request_host_for_metric.as_deref()) {
        crate::app::APP_CTX.rps.inc_domain(domain);
    }

    if location.proxy_pass_to.is_drop() {
        return ReaderStep::Close;
    }

    let identity = match h1_reader
        .authorize(end_point_info, location, http_connection_info, &request_headers)
        .await
    {
        Ok(identity) => identity,
        Err(err) => {
            return respond_error(queue_tx, http_connection_info, Some(&endpoint_for_error), err)
                .await
        }
    };

    if !end_point_info.user_is_allowed(&identity).await {
        return respond_error(
            queue_tx,
            http_connection_info,
            Some(&endpoint_for_error),
            ProxyServerError::NotAuthorized,
        )
        .await;
    }

    // Resolve dynamic_proxy → a per-request synthetic upstream + Host override.
    let (synthetic_proxy_pass_to, dynamic_host_override): (Option<ProxyPassToConfig>, Option<String>) =
        match &location.proxy_pass_to {
            ProxyPassToConfig::DynamicProxy(config) => {
                let buf = h1_reader.loop_buffer.get_data();
                let proxy_to = match request_headers.find_header_value_str(buf, b"proxy-to") {
                    Some(v) => v.to_string(),
                    None => {
                        return respond_error(
                            queue_tx,
                            http_connection_info,
                            Some(&endpoint_for_error),
                            ProxyServerError::ProxyToHeaderMissing,
                        )
                        .await
                    }
                };
                let endpoint =
                    match rust_extensions::remote_endpoint::RemoteEndpointOwned::try_parse(proxy_to) {
                        Ok(e) => e,
                        Err(_) => {
                            return respond_error(
                                queue_tx,
                                http_connection_info,
                                Some(&endpoint_for_error),
                                ProxyServerError::ProxyToHeaderInvalid,
                            )
                            .await
                        }
                    };
                use rust_extensions::remote_endpoint::Scheme;
                match endpoint.get_scheme() {
                    Some(Scheme::Http) | Some(Scheme::Https) | Some(Scheme::Ws) | Some(Scheme::Wss) => {}
                    _ => {
                        return respond_error(
                            queue_tx,
                            http_connection_info,
                            Some(&endpoint_for_error),
                            ProxyServerError::ProxyToHeaderInvalid,
                        )
                        .await
                    }
                }
                if let Some(allowed) = &config.allowed_hosts {
                    let host = endpoint.get_host();
                    if !allowed.iter().any(|h| h.eq_ignore_ascii_case(host)) {
                        return respond_error(
                            queue_tx,
                            http_connection_info,
                            Some(&endpoint_for_error),
                            ProxyServerError::ProxyToHostNotAllowed,
                        )
                        .await;
                    }
                }
                let host_port = endpoint.get_host_port().to_string();
                let synth = ProxyPassToConfig::Http1(ProxyPassToModel {
                    remote_host: MyReverseProxyRemoteEndpoint::Direct {
                        remote_host: Arc::new(endpoint),
                    },
                    request_timeout: config.request_timeout,
                    connect_timeout: config.connect_timeout,
                    pool_tuning: crate::configurations::PoolTuning::default(),
                });
                (Some(synth), Some(host_port))
            }
            _ => (None, None),
        };

    let proxy_pass_to_owned: ProxyPassToConfig = synthetic_proxy_pass_to
        .unwrap_or_else(|| location.proxy_pass_to.clone());

    let content_length = request_headers.content_length;

    let compiled = h1_reader.compile_headers(
        request_headers,
        H1HeadersKind::Request(end_point_info),
        http_connection_info,
        &identity,
        mcp_path(&proxy_pass_to_owned),
        dynamic_host_override.as_deref(),
    );
    let is_websocket = match compiled {
        Ok(ws) => ws,
        Err(err) => {
            return respond_error(queue_tx, http_connection_info, Some(&endpoint_for_error), err)
                .await
        }
    };

    // Static / local-files: synthesize the response without an upstream. Drain
    // the request body first so the connection stays byte-synced for reuse.
    if let ProxyPassToConfig::Static(cfg) = &location.proxy_pass_to {
        let mut builder = crate::h1_utils::Http1ResponseBuilder::new(cfg.status_code);
        if let Some(content_type) = cfg.content_type.as_ref() {
            builder = builder.add_content_type(content_type);
        }
        let bytes = builder.build_with_body(&cfg.body);
        let mut null = super::NullSink;
        let _ = h1_reader.transfer_body(0, &mut null, content_length).await;
        return match emit_single_response(queue_tx, write_timeout, bytes).await {
            ReaderStep::Continue if !keep_alive => ReaderStep::Close,
            other => other,
        };
    }
    if let ProxyPassToConfig::FilesPath(model) = &location.proxy_pass_to {
        // Serve a file from the configured folder (README "Serving the folder
        // with files"): the request path picks the file; default_file is served
        // for "/". get_content returns a full HTTP response (or NOT_FOUND /
        // non-GET 404). Drain the request body so the connection stays synced.
        let content_source = crate::http_content_source::local_path::LocalPathContent::new(
            model.files_path.to_string().as_str(),
            model.default_file.clone(),
        );
        content_source.send_headers(0, &h1_reader.h1_headers_builder);
        let response = content_source.get_content(0).await;
        let bytes = response.as_slice().to_vec();
        let mut null = super::NullSink;
        let _ = h1_reader.transfer_body(0, &mut null, content_length).await;
        return match emit_single_response(queue_tx, write_timeout, bytes).await {
            ReaderStep::Continue if !keep_alive => ReaderStep::Close,
            other => other,
        };
    }

    let head = h1_reader.h1_headers_builder.as_slice().to_vec();

    // Clone the owned context before the borrows end.
    let end_point_info = end_point_info.clone();
    let location_id = location.id;
    let conn_info = http_connection_info.clone();

    if is_websocket {
        // Upgrade is terminal for the connection — hand a context back to the
        // connection entry, which reclaims the client write half and tunnels.
        // (No request body on a websocket handshake.)
        return ReaderStep::Upgrade(UpgradeContext {
            proxy_pass_to: proxy_pass_to_owned,
            end_point_info,
            http_connection_info: conn_info,
            location_id,
            head,
            write_timeout,
        });
    }

    // Reserve this request's ordered output slot, then spawn its worker.
    let (response_tx, response_rx) = mpsc::channel::<ResponseEvent>(RESPONSE_CHANNEL_CAPACITY);
    let (body_tx, body_rx) = mpsc::channel::<Vec<u8>>(REQUEST_BODY_CHANNEL_CAPACITY);

    if queue_tx
        .send(ResponseSlot {
            events: response_rx,
            write_timeout,
        })
        .await
        .is_err()
    {
        // Writer is gone — connection is finished.
        return ReaderStep::Close;
    }

    crate::app::spawn_named(
        "h1_upstream_worker",
        run_upstream_request(UpstreamRequest {
            pool: pool.clone(),
            proxy_pass_to: proxy_pass_to_owned,
            end_point_info,
            http_connection_info: conn_info,
            location_id,
            head,
            body_rx,
            response_tx,
        }),
    );

    // Stream the request body to the worker. When done, drop body_tx so the
    // worker's body_rx closes.
    let mut body_sink = BodyChannelSink::new(body_tx);
    match h1_reader
        .transfer_body(0, &mut body_sink, content_length)
        .await
    {
        Ok(_) if keep_alive => ReaderStep::Continue,
        // keep_alive=false endpoint: close after this request.
        Ok(_) => ReaderStep::Close,
        // Worker gone or client body read failed → connection desynced, close.
        Err(_) => ReaderStep::Close,
    }
}

/// Push a single fully-formed response (already-built bytes) as one ordered
/// slot and keep the connection open. Returns `Close` only if the writer is gone.
async fn emit_single_response(
    queue_tx: &mpsc::Sender<ResponseSlot>,
    write_timeout: Duration,
    bytes: Vec<u8>,
) -> ReaderStep {
    let (tx, rx) = mpsc::channel::<ResponseEvent>(2);
    if queue_tx
        .send(ResponseSlot {
            events: rx,
            write_timeout,
        })
        .await
        .is_err()
    {
        return ReaderStep::Close;
    }
    if tx.send(ResponseEvent::Chunk(bytes)).await.is_ok() {
        let _ = tx.send(ResponseEvent::Done).await;
    }
    ReaderStep::Continue
}

/// Push an ordered error response (per `ProxyServerError::error_handling`) and
/// close the connection.
async fn respond_error(
    queue_tx: &mpsc::Sender<ResponseSlot>,
    http_connection_info: &HttpConnectionInfo,
    endpoint: Option<&str>,
    err: ProxyServerError,
) -> ReaderStep {
    let handling = err.error_handling();

    if handling.register_ip_failure {
        if let Some(ip) = http_connection_info.connection_ip.get_ip_addr() {
            crate::app::APP_CTX.ip_blocklist.register_failure(ip);
        }
    }

    if let (Some(status), Some(endpoint)) = (handling.status_5xx, endpoint) {
        crate::app::APP_CTX.proxy_logs.write_returned_5xx(
            endpoint,
            None,
            http_connection_info.connection_ip.get_ip_log(),
            status,
            format!("{:?}", err),
        );
    }

    if let Some(page) = handling.page {
        let (tx, rx) = mpsc::channel::<ResponseEvent>(2);
        if queue_tx
            .send(ResponseSlot {
                events: rx,
                write_timeout: crate::consts::DEFAULT_WRITE_TIMEOUT,
            })
            .await
            .is_ok()
        {
            if tx.send(ResponseEvent::Chunk(page.to_vec())).await.is_ok() {
                let _ = tx.send(ResponseEvent::Done).await;
            }
        }
    }

    ReaderStep::Close
}
