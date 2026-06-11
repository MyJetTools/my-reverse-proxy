use std::cell::RefCell;
use std::sync::Arc;

use crate::configurations::{MyReverseProxyRemoteEndpoint, ProxyPassToConfig, ProxyPassToModel};
use crate::network_stream::*;

use super::*;

use crate::h1_remote_connection::*;

struct Http1ConnectionGauge {
    listen_addr: String,
    port: Option<u16>,
    /// The configured endpoint host string (e.g. `"myapp.com:443"`) once it's
    /// known. For HTTPS this is set right after the TLS handshake; for plain
    /// HTTP it's set lazily after the first request resolves the Host header.
    /// `RefCell` because the gauge value moves with the connection but we
    /// learn the endpoint inside the request loop.
    endpoint: RefCell<Option<String>>,
}

impl Http1ConnectionGauge {
    fn attribute_to_endpoint(&self, host: &str) {
        let mut slot = self.endpoint.borrow_mut();
        if slot.is_some() {
            return;
        }
        crate::app::APP_CTX
            .metrics
            .update(|m| m.connection_by_endpoint.inc(&host.to_string()));
        *slot = Some(host.to_string());
    }
}

impl Drop for Http1ConnectionGauge {
    fn drop(&mut self) {
        crate::app::APP_CTX
            .prometheus
            .dec_http1_server_connections(&self.listen_addr);
        if let Some(port) = self.port {
            crate::app::APP_CTX
                .metrics
                .update(|m| m.connection_by_port.dec(&port));
        }
        if let Some(endpoint) = self.endpoint.borrow().as_ref() {
            crate::app::APP_CTX
                .metrics
                .update(|m| m.connection_by_endpoint.dec(endpoint));
        }
    }
}

pub async fn serve_reverse_proxy<
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
    let conn_gauge = Http1ConnectionGauge {
        listen_addr,
        port,
        endpoint: RefCell::new(None),
    };
    // For HTTPS / TLS-h1, the endpoint is already resolved by SNI at the
    // handshake — record it now so we don't depend on the first request.
    if let Some(endpoint_info) = http_connection_info.endpoint_info.as_ref() {
        conn_gauge.attribute_to_endpoint(endpoint_info.host_endpoint.as_str());
    }

    let mut upstream_state = UpstreamState::new();

    let (server_read_part, server_write_part) = server_stream.split();

    // Transport timeouts are endpoint-scoped. For TLS the endpoint is known via
    // SNI; for plain HTTP it resolves on the first request, so fall back to the
    // defaults until then.
    let timeouts = http_connection_info
        .endpoint_info
        .as_ref()
        .map(|e| e.timeouts)
        .unwrap_or_default();

    let mut h1_reader = H1Reader::new(server_read_part, timeouts);

    let h1_server_write_part = H1ServerWritePart::new(server_write_part);

    let mut request_id = 0;

    loop {
        request_id += 1;
        // After the first request resolves the endpoint via Host header on
        // plain HTTP, attribute the connection so per-domain counters tick.
        if let Some(endpoint_info) = http_connection_info.endpoint_info.as_ref() {
            conn_gauge.attribute_to_endpoint(endpoint_info.host_endpoint.as_str());
        }
        let execute_request_result = execute_request(
            &mut http_connection_info,
            &mut h1_reader,
            &mut upstream_state,
            &h1_server_write_part,
        )
        .await;

        match execute_request_result {
            Ok(web_socket_upgrade) => {
                if let Some(web_socket_upgrade) = web_socket_upgrade {
                    if let Some(upstream) =
                        upstream_state.take(web_socket_upgrade.upstream_key.as_str())
                    {
                        let (server_read_part, loop_buffer) = h1_reader.into_read_part();

                        h1_server_write_part
                            .add_web_socket_upgrade(upstream, server_read_part, loop_buffer)
                            .await;

                        return;
                    }
                } else {
                    let close_connection = http_connection_info
                        .endpoint_info
                        .as_ref()
                        .map(|e| !e.keep_alive)
                        .unwrap_or(false);
                    if close_connection {
                        break;
                    }
                }
            }
            Err(err) => {
                let content = match &err {
                    ProxyServerError::NetworkError(_) => {
                        break;
                    }
                    ProxyServerError::DropConnection => {
                        break;
                    }
                    ProxyServerError::HttpConfigurationIsNotFound
                    | ProxyServerError::ParsingPayloadError(_)
                    | ProxyServerError::ChunkHeaderParseError
                    | ProxyServerError::HeadersParseError(_) => {
                        if let Some(ip) = http_connection_info.connection_ip.get_ip_addr() {
                            crate::app::APP_CTX.ip_blocklist.register_failure(ip);
                        }
                        break;
                    }
                    ProxyServerError::BufferAllocationFail => {
                        println!("Buffer allocation fail - server loop");
                        crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
                    }
                    ProxyServerError::CanNotConnectToRemoteResource {
                        remote_resource: _,
                        err,
                    } => {
                        if err.as_timeout().is_some() {
                            crate::error_templates::ERROR_TIMEOUT.as_slice()
                        } else {
                            crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
                        }
                    }
                    ProxyServerError::LocationIsNotFound => {
                        crate::error_templates::LOCATION_IS_NOT_FOUND.as_slice()
                    }
                    ProxyServerError::CanNotWriteContentToRemoteConnection(err) => {
                        println!("Can not write to remote resource. Err: {:?}", err);
                        crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
                    }
                    ProxyServerError::NotAuthorized => {
                        crate::error_templates::NOT_AUTHORIZED_PAGE.as_slice()
                    }
                    ProxyServerError::ProxyToHeaderMissing
                    | ProxyServerError::ProxyToHeaderInvalid => {
                        crate::error_templates::PROXY_TO_HEADER_MISSING.as_slice()
                    }
                    ProxyServerError::ProxyToHostNotAllowed => {
                        crate::error_templates::PROXY_TO_HOST_NOT_ALLOWED.as_slice()
                    }
                    ProxyServerError::HttpResponse(payload) => payload.as_slice(),
                };

                // Every 5xx returned to the client is always recorded.
                let status_5xx = match &err {
                    ProxyServerError::BufferAllocationFail
                    | ProxyServerError::CanNotConnectToRemoteResource { .. }
                    | ProxyServerError::CanNotWriteContentToRemoteConnection(_)
                    | ProxyServerError::LocationIsNotFound => Some(503u16),
                    _ => None,
                };
                if let Some(status) = status_5xx {
                    if let Some(endpoint_info) = http_connection_info.endpoint_info.as_ref() {
                        crate::app::APP_CTX.proxy_logs.write_returned_5xx(
                            endpoint_info.host_endpoint.as_str(),
                            None,
                            http_connection_info.connection_ip.get_ip_log(),
                            status,
                            format!("{:?}", err),
                        );
                    }
                }

                let write_timeout = http_connection_info
                    .endpoint_info
                    .as_ref()
                    .map(|e| e.timeouts.write_timeout)
                    .unwrap_or(crate::consts::DEFAULT_WRITE_TIMEOUT);
                let _ = h1_server_write_part
                    .write_http_payload_with_timeout(request_id, content, write_timeout)
                    .await;

                // The failed request was never consumed from the read buffer
                // (read_headers only peeks; consumption happens during
                // forwarding). Looping again would re-parse the same bytes and
                // spin at machine speed producing the same error page. The
                // connection is desynced anyway — close it.
                break;
            }
        }
    }
}

async fn execute_request<
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
>(
    http_connection_info: &mut HttpConnectionInfo,
    h1_reader: &mut H1Reader<ReadPart>,
    upstream_state: &mut UpstreamState,
    h1_server_write_part: &H1ServerWritePart<WritePart, ReadPart>,
) -> Result<Option<WebSocketUpgradeResult>, ProxyServerError> {
    let request_headers = h1_reader.read_headers().await?;

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

    let (location, end_point_info) = h1_reader
        .find_location(&request_headers, &http_connection_info)
        .await?;

    // Now that the endpoint is known, apply its transport read/write timeouts to
    // the reader (for plain HTTP it started on defaults before the first request).
    h1_reader.timeouts = end_point_info.timeouts;

    if let Some(domain) = end_point_info.tracked_domain(request_host_for_metric.as_deref()) {
        crate::app::APP_CTX.rps.inc_domain(domain);
    }

    if location.proxy_pass_to.is_drop() {
        return Err(ProxyServerError::DropConnection);
    }

    let identity = h1_reader
        .authorize(
            end_point_info,
            location,
            &http_connection_info,
            &request_headers,
        )
        .await?;

    if !end_point_info.user_is_allowed(&identity).await {
        return Err(ProxyServerError::NotAuthorized);
    }

    let http_connection_context = Http1ServerConnectionContext {
        h1_server_write_part: h1_server_write_part.clone(),
        http_connection_info: http_connection_info.clone(),
        end_point_info: end_point_info.clone(),
        location_id: location.id,
    };

    // Resolve dynamic_proxy: rewrite proxy_pass_to per-request from the
    // `proxy-to` header, and force fresh upstream connect (no pooling).
    let synthetic_proxy_pass_to: Option<ProxyPassToConfig>;
    let dynamic_host_override: Option<String>;
    match &location.proxy_pass_to {
        ProxyPassToConfig::DynamicProxy(config) => {
            let buf = h1_reader.loop_buffer.get_data();
            let proxy_to = request_headers
                .find_header_value_str(buf, b"proxy-to")
                .ok_or(ProxyServerError::ProxyToHeaderMissing)?
                .to_string();
            let endpoint =
                rust_extensions::remote_endpoint::RemoteEndpointOwned::try_parse(proxy_to)
                    .map_err(|_| ProxyServerError::ProxyToHeaderInvalid)?;
            use rust_extensions::remote_endpoint::Scheme;
            match endpoint.get_scheme() {
                Some(Scheme::Http) | Some(Scheme::Https) | Some(Scheme::Ws) | Some(Scheme::Wss) => {
                }
                _ => return Err(ProxyServerError::ProxyToHeaderInvalid),
            }
            if let Some(allowed) = &config.allowed_hosts {
                let host = endpoint.get_host();
                if !allowed.iter().any(|h| h.eq_ignore_ascii_case(host)) {
                    return Err(ProxyServerError::ProxyToHostNotAllowed);
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
            // No pre-discard needed: upstreams are keyed by remote host, so a
            // different proxy-to target maps to its own entry, and the same
            // host is reused on purpose.
            synthetic_proxy_pass_to = Some(synth);
            dynamic_host_override = Some(host_port);
        }
        _ => {
            synthetic_proxy_pass_to = None;
            dynamic_host_override = None;
        }
    }
    let effective_proxy_pass_to: &ProxyPassToConfig = synthetic_proxy_pass_to
        .as_ref()
        .unwrap_or(&location.proxy_pass_to);

    let access = upstream_state
        .get_or_connect(effective_proxy_pass_to, &http_connection_context)
        .await
        .map_err(|err| {
            log_upstream_failure(
                location.id,
                http_connection_info,
                format!(
                    "Can not connect to upstream {}: {:?}",
                    effective_proxy_pass_to.to_string(),
                    err
                ),
            );
            ProxyServerError::CanNotConnectToRemoteResource {
                remote_resource: effective_proxy_pass_to.to_string(),
                err,
            }
        })?;

    let content_length = request_headers.content_length;

    let web_socket_upgrade = h1_reader.compile_headers(
        request_headers,
        H1HeadersKind::Request(end_point_info),
        &http_connection_info,
        &identity,
        access.mcp_path,
        dynamic_host_override.as_deref(),
    )?;

    let send_headers_result = access
        .upstream
        .send_h1_header(
            &h1_reader.h1_headers_builder,
            end_point_info.timeouts.write_timeout,
        )
        .await;

    let mut upstream = access.upstream;

    if !send_headers_result {
        upstream_state.mark_disposed(effective_proxy_pass_to);

        println!("Doing reconnection to remote connection");

        let access = upstream_state
            .get_or_connect(effective_proxy_pass_to, &http_connection_context)
            .await
            .map_err(|err| {
                log_upstream_failure(
                    location.id,
                    http_connection_info,
                    format!(
                        "Can not reconnect to upstream {}: {:?}",
                        effective_proxy_pass_to.to_string(),
                        err
                    ),
                );
                ProxyServerError::CanNotConnectToRemoteResource {
                    remote_resource: effective_proxy_pass_to.to_string(),
                    err,
                }
            })?;

        let send_headers_result = access
            .upstream
            .send_h1_header(
                &h1_reader.h1_headers_builder,
                end_point_info.timeouts.write_timeout,
            )
            .await;

        if !send_headers_result {
            log_upstream_failure(
                location.id,
                http_connection_info,
                format!(
                    "Upstream {} reconnected but sending request headers failed again",
                    effective_proxy_pass_to.to_string()
                ),
            );
            return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
                NetworkError::OtherStr("Remote resource is disconnected"),
            ));
        }

        upstream = access.upstream;
    }

    if web_socket_upgrade {
        return Ok(Some(WebSocketUpgradeResult {
            upstream_key: crate::h1_remote_connection::connection_key(effective_proxy_pass_to),
        }));
    }

    h1_server_write_part
        .add_current_request(upstream.connection_id)
        .await;

    let bytes_to_upstream = match h1_reader
        .transfer_body(upstream.connection_id, upstream, content_length)
        .await
    {
        Ok(bytes) => bytes,
        Err(err) => {
            // Remote may have received partial body — connection is desynced
            upstream_state.mark_disposed(effective_proxy_pass_to);
            log_upstream_failure(
                location.id,
                http_connection_info,
                format!(
                    "Upstream {} request failed transferring request body: {:?}",
                    effective_proxy_pass_to.to_string(),
                    err
                ),
            );
            return Err(err);
        }
    };

    crate::app::APP_CTX.traffic.record_c2s(
        end_point_info.host_endpoint.as_str(),
        bytes_to_upstream as u64,
    );

    let connected = upstream.read_http_response(http_connection_context);

    if !connected {
        return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
            NetworkError::OtherStr("Remote connection is lost"),
        ));
    }

    // Note: do NOT drop the upstream here even on success — the response for
    // this request is still being pumped by the background
    // `response_read_loop`; dropping the cached Upstream would abort the loop
    // and the client would never see the response. A broken entry is only
    // marked disposed and gets recreated by the next get_or_connect.

    Ok(None)
}

pub struct WebSocketUpgradeResult {
    upstream_key: String,
}

/// Upstream-reach failures are always recorded at the location scope — same
/// contract as the hyper path.
fn log_upstream_failure(
    location_id: i64,
    http_connection_info: &HttpConnectionInfo,
    message: String,
) {
    crate::app::APP_CTX.proxy_logs.write_location(
        location_id,
        http_connection_info.connection_ip.get_ip_log(),
        message,
    );
}
