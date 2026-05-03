use crate::network_stream::*;

use super::*;

use crate::h1_remote_connection::*;

struct Http1ConnectionGauge(String);

impl Drop for Http1ConnectionGauge {
    fn drop(&mut self) {
        crate::app::APP_CTX
            .prometheus
            .dec_http1_server_connections(&self.0);
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
    crate::app::APP_CTX
        .prometheus
        .inc_http1_server_connections(&listen_addr);
    let _conn_gauge = Http1ConnectionGauge(listen_addr);

    let mut upstream_state = UpstreamState::new();

    let (server_read_part, server_write_part) = server_stream.split();

    let timeouts = crate::types::HttpTimeouts {
        read_timeout: crate::consts::READ_TIMEOUT,
    };

    let mut h1_reader = H1Reader::new(server_read_part, timeouts);

    let h1_server_write_part = H1ServerWritePart::new(server_write_part);

    let mut request_id = 0;

    loop {
        request_id += 1;
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
                        upstream_state.take_http(web_socket_upgrade.location_id)
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
                let close_after = matches!(&err, ProxyServerError::LocationIsNotFound);

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
                    ProxyServerError::HttpResponse(payload) => payload.as_slice(),
                };
                let _ = h1_server_write_part
                    .write_http_payload_with_timeout(
                        request_id,
                        content,
                        crate::consts::WRITE_TIMEOUT,
                    )
                    .await;

                if close_after {
                    break;
                }
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

    if http_connection_info.endpoint_info.is_none() {
        http_connection_info.endpoint_info =
            h1_reader.try_find_endpoint_info(&request_headers, &http_connection_info.listen_config);

        if http_connection_info.endpoint_info.is_some() {
            if let Some(ip) = http_connection_info.connection_ip.get_ip_addr() {
                crate::app::APP_CTX.ip_blocklist.register_success(&ip);
            }
        }
    }

    let (location, end_point_info) = h1_reader
        .find_location(&request_headers, &http_connection_info)
        .await?;

    if let Some(domain) = end_point_info.tracked_domain(request_host_for_metric.as_deref()) {
        crate::app::APP_CTX.rps.inc_domain(domain);
    }

    if location.proxy_pass_to.is_drop() {
        return Err(ProxyServerError::DropConnection);
    }

    let identity = h1_reader
        .authorize(end_point_info, location, &http_connection_info, &request_headers)
        .await?;

    if !end_point_info.user_is_allowed(&identity).await {
        return Err(ProxyServerError::NotAuthorized);
    }

    let http_connection_context = Http1ServerConnectionContext {
        h1_server_write_part: h1_server_write_part.clone(),
        http_connection_info: http_connection_info.clone(),
        end_point_info: end_point_info.clone(),
    };

    let access = upstream_state
        .get_or_connect(&location.proxy_pass_to, location.id, &http_connection_context)
        .await
        .map_err(|err| ProxyServerError::CanNotConnectToRemoteResource {
            remote_resource: location.proxy_pass_to.to_string(),
            err,
        })?;

    let content_length = request_headers.content_length;

    let web_socket_upgrade = h1_reader.compile_headers(
        request_headers,
        H1HeadersKind::Request(end_point_info),
        &http_connection_info,
        &identity,
        access.mcp_path,
    )?;

    let send_headers_result = access
        .upstream
        .send_h1_header(&h1_reader.h1_headers_builder, crate::consts::WRITE_TIMEOUT)
        .await;

    let mut upstream = access.upstream;

    if !send_headers_result {
        upstream_state.discard(&location.proxy_pass_to, location.id);

        println!("Doing reconnection to remote connection");

        let access = upstream_state
            .get_or_connect(&location.proxy_pass_to, location.id, &http_connection_context)
            .await
            .map_err(|err| ProxyServerError::CanNotConnectToRemoteResource {
                remote_resource: location.proxy_pass_to.to_string(),
                err,
            })?;

        let send_headers_result = access
            .upstream
            .send_h1_header(&h1_reader.h1_headers_builder, crate::consts::WRITE_TIMEOUT)
            .await;

        if !send_headers_result {
            return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
                NetworkError::OtherStr("Remote resource is disconnected"),
            ));
        }

        upstream = access.upstream;
    }

    if web_socket_upgrade {
        return Ok(Some(WebSocketUpgradeResult {
            location_id: location.id,
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
            upstream_state.discard(&location.proxy_pass_to, location.id);
            return Err(err);
        }
    };

    crate::app::APP_CTX
        .traffic
        .record_c2s(end_point_info.host_endpoint.as_str(), bytes_to_upstream as u64);

    let connected = upstream.read_http_response(http_connection_context);

    if !connected {
        return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
            NetworkError::OtherStr("Remote connection is lost"),
        ));
    }

    Ok(None)
}

pub struct WebSocketUpgradeResult {
    location_id: i64,
}
