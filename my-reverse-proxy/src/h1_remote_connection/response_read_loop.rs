use std::sync::Arc;

use rust_extensions::UnsafeValue;

use super::*;

use crate::{
    app::SshSessionHandler,
    h1_proxy_server::{H1HeadersKind, H1Reader, H1Writer},
    network_stream::*,
};

pub async fn response_read_loop<
    RemoteReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    ServerWritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ServerReadPart: NetworkStreamReadPart + Send + Sync + 'static,
>(
    connection_id: u64,
    remote_read_part: RemoteReadPart,
    remote_disconnected: Arc<UnsafeValue<bool>>,
    mut server_write_part: Http1ServerConnectionContext<ServerWritePart, ServerReadPart>,
    ssh_session_handler: Option<SshSessionHandler>,
) {
    // Endpoint-scoped transport timeouts, applied to both reading the upstream
    // response and writing it back to the client.
    let timeouts = server_write_part.end_point_info.timeouts;

    let mut remote_h1_reader = H1Reader::new(remote_read_part, timeouts);

    loop {
        let resp_headers = match remote_h1_reader.read_headers().await {
            Ok(headers) => headers,
            Err(err) => {
                println!("Reading header from remote: {:?}", err);

                remote_disconnected.set_value(true);

                let _ = server_write_part
                    .h1_server_write_part
                    .write_http_payload_with_timeout(
                        connection_id,
                        crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE
                            .as_slice(),
                        timeouts.write_timeout,
                    )
                    .await;

                server_write_part
                    .h1_server_write_part
                    .request_is_done(connection_id, timeouts.write_timeout)
                    .await;
                return;
            }
        };

        let content_length = resp_headers.content_length;

        let web_socket_upgrade = match remote_h1_reader.compile_headers(
            resp_headers,
            H1HeadersKind::Response(&server_write_part.end_point_info),
            &server_write_part.http_connection_info,
            &None,
            None,
            None,
        ) {
            Ok(web_socket_upgrade) => web_socket_upgrade,
            Err(err) => {
                println!("Compile headers from remote: {:?}", err);

                remote_disconnected.set_value(true);
                let _ = server_write_part
                    .h1_server_write_part
                    .write_http_payload_with_timeout(
                        connection_id,
                        crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE
                            .as_slice(),
                        timeouts.write_timeout,
                    )
                    .await;

                server_write_part
                    .h1_server_write_part
                    .request_is_done(connection_id, timeouts.write_timeout)
                    .await;
                return;
            }
        };

        if let Err(err) = server_write_part
            .h1_server_write_part
            .write_http_payload_with_timeout(
                connection_id,
                remote_h1_reader.h1_headers_builder.as_slice(),
                timeouts.write_timeout,
            )
            .await
        {
            println!("Sending headers from remote to server: {:?}", err);

            remote_disconnected.set_value(true);

            let _ = server_write_part
                .h1_server_write_part
                .write_http_payload_with_timeout(
                    connection_id,
                    crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                    timeouts.write_timeout,
                )
                .await;

            server_write_part
                .h1_server_write_part
                .request_is_done(connection_id, timeouts.write_timeout)
                .await;
            return;
        }

        let bytes_to_client = match remote_h1_reader
            .transfer_body(
                connection_id,
                &mut server_write_part.h1_server_write_part,
                content_length,
            )
            .await
        {
            Ok(bytes) => bytes,
            Err(err) => {
                println!("Sending body from remote to server: {:?}", err);

                remote_disconnected.set_value(true);

                let _ = server_write_part
                    .h1_server_write_part
                    .write_http_payload(
                        connection_id,
                        crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE
                            .as_slice(),
                        timeouts.write_timeout,
                    )
                    .await;

                server_write_part
                    .h1_server_write_part
                    .request_is_done(connection_id, timeouts.write_timeout)
                    .await;
                return;
            }
        };

        crate::app::APP_CTX.traffic.record_s2c(
            server_write_part.end_point_info.host_endpoint.as_str(),
            bytes_to_client as u64,
        );

        server_write_part
            .h1_server_write_part
            .request_is_done(connection_id, timeouts.write_timeout)
            .await;

        if web_socket_upgrade {
            let (remote_read_part, remote_loop_buffer) = remote_h1_reader.into_read_part();

            let ws_domain = server_write_part
                .end_point_info
                .host_endpoint
                .as_str()
                .to_string();

            // Capture the log scope (endpoint + location + client IP) before the
            // upgrade consumes the connection context, so pump errors land in the
            // right location's in-memory log instead of the console.
            let log_scope = crate::app::ProxyLogScope::new(
                std::sync::Arc::new(ws_domain.clone()),
                server_write_part.location_id,
                server_write_part
                    .http_connection_info
                    .connection_ip
                    .get_ip_log(),
            );

            let (mut server_write_part, server_web_socket_data) = server_write_part
                .h1_server_write_part
                .upgrade_web_socket()
                .await;

            let make_s2c_recorder = || {
                Some(crate::tcp_utils::WsTrafficRecorder {
                    domain: ws_domain.clone(),
                    direction: crate::tcp_utils::WsDirection::ServerToClient,
                })
            };
            let make_c2s_recorder = || {
                Some(crate::tcp_utils::WsTrafficRecorder {
                    domain: ws_domain.clone(),
                    direction: crate::tcp_utils::WsDirection::ClientToServer,
                })
            };

            match server_web_socket_data.upstream.inner {
                UpstreamInner::Http1Direct(inner) => {
                    crate::app::spawn_named(
                        "h1_ws_pump_client_to_server_http1direct",
                        crate::tcp_utils::copy_streams(
                            remote_read_part,
                            server_write_part,
                            remote_loop_buffer,
                            ssh_session_handler,
                            make_s2c_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );

                    crate::app::spawn_named(
                        "h1_ws_pump_server_to_client_http1direct",
                        crate::tcp_utils::copy_streams(
                            server_web_socket_data.read_part,
                            inner.remote_write_part,
                            server_web_socket_data.loop_buffer,
                            None,
                            make_c2s_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );
                }
                UpstreamInner::Http1UnixSocket(inner) => {
                    crate::app::spawn_named(
                        "h1_ws_pump_client_to_server_http1unix",
                        crate::tcp_utils::copy_streams(
                            remote_read_part,
                            server_write_part,
                            remote_loop_buffer,
                            ssh_session_handler,
                            make_s2c_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );

                    crate::app::spawn_named(
                        "h1_ws_pump_server_to_client_http1unix",
                        crate::tcp_utils::copy_streams(
                            server_web_socket_data.read_part,
                            inner.remote_write_part,
                            server_web_socket_data.loop_buffer,
                            None,
                            make_c2s_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );
                }
                UpstreamInner::Https1Direct(inner) => {
                    crate::app::spawn_named(
                        "h1_ws_pump_client_to_server_https",
                        crate::tcp_utils::copy_streams(
                            remote_read_part,
                            server_write_part,
                            remote_loop_buffer,
                            ssh_session_handler,
                            make_s2c_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );

                    crate::app::spawn_named(
                        "h1_ws_pump_server_to_client_https",
                        crate::tcp_utils::copy_streams(
                            server_web_socket_data.read_part,
                            inner.remote_write_part,
                            server_web_socket_data.loop_buffer,
                            None,
                            make_c2s_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );
                }
                UpstreamInner::Http1OverSsh(inner) => {
                    crate::app::spawn_named(
                        "h1_ws_pump_client_to_server_ssh",
                        crate::tcp_utils::copy_streams(
                            remote_read_part,
                            server_write_part,
                            remote_loop_buffer,
                            ssh_session_handler,
                            make_s2c_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );

                    crate::app::spawn_named(
                        "h1_ws_pump_server_to_client_ssh",
                        crate::tcp_utils::copy_streams(
                            server_web_socket_data.read_part,
                            inner.remote_write_part,
                            server_web_socket_data.loop_buffer,
                            None,
                            make_c2s_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );
                }
                UpstreamInner::Http1OverGateway(inner) => {
                    crate::app::spawn_named(
                        "h1_ws_pump_client_to_server_gateway",
                        crate::tcp_utils::copy_streams(
                            remote_read_part,
                            server_write_part,
                            remote_loop_buffer,
                            ssh_session_handler,
                            make_s2c_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );
                    crate::app::spawn_named(
                        "h1_ws_pump_server_to_client_gateway",
                        crate::tcp_utils::copy_streams(
                            server_web_socket_data.read_part,
                            inner.remote_write_part,
                            server_web_socket_data.loop_buffer,
                            None,
                            make_c2s_recorder(),
                            Some(log_scope.clone()),
                            timeouts,
                        ),
                    );
                }
                UpstreamInner::StaticContent { .. } => {
                    let _ = server_write_part
                        .write_to_socket(
                            crate::error_templates::ENDPOINT_CAN_NOT_BE_UPGRADED_TO_WEB_SOCKET
                                .as_slice(),
                        )
                        .await;
                    let _ = server_write_part.shutdown_socket().await;
                }
                UpstreamInner::LocalFiles { .. } => {
                    let _ = server_write_part
                        .write_to_socket(
                            crate::error_templates::ENDPOINT_CAN_NOT_BE_UPGRADED_TO_WEB_SOCKET
                                .as_slice(),
                        )
                        .await;
                    let _ = server_write_part.shutdown_socket().await;
                }
            }

            return;
        }
    }
}
