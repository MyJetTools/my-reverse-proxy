use std::sync::Arc;

use rust_extensions::UnsafeValue;

use super::*;

use crate::{
    app::SshSessionHandler,
    h1_proxy_server::{H1Reader, H1Writer},
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
    println!(
        "Started read loop {}",
        server_write_part.http_connection_info.listening_addr
    );
    let mut remote_h1_reader =
        H1Reader::new(remote_read_part, crate::types::HttpTimeouts::default());
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
                        crate::consts::WRITE_TIMEOUT,
                    )
                    .await;

                server_write_part
                    .h1_server_write_part
                    .request_is_done(connection_id)
                    .await;
                return;
            }
        };

        let content_length = resp_headers.content_length;

        println!("Resp content len: {:?}", content_length);

        let web_socket_upgrade = match remote_h1_reader.compile_headers(
            resp_headers,
            &server_write_part.end_point_info.modify_response_headers,
            &server_write_part.http_connection_info,
            &None,
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
                        crate::consts::WRITE_TIMEOUT,
                    )
                    .await;

                server_write_part
                    .h1_server_write_part
                    .request_is_done(connection_id)
                    .await;
                return;
            }
        };

        if let Err(err) = server_write_part
            .h1_server_write_part
            .write_http_payload_with_timeout(
                connection_id,
                remote_h1_reader.h1_headers_builder.as_slice(),
                crate::consts::WRITE_TIMEOUT,
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
                    crate::consts::WRITE_TIMEOUT,
                )
                .await;

            server_write_part
                .h1_server_write_part
                .request_is_done(connection_id)
                .await;
            return;
        }

        if let Err(err) = remote_h1_reader
            .transfer_body(
                connection_id,
                &mut server_write_part.h1_server_write_part,
                content_length,
            )
            .await
        {
            println!("Sending body from remote to server: {:?}", err);

            remote_disconnected.set_value(true);

            let _ = server_write_part
                .h1_server_write_part
                .write_http_payload(
                    connection_id,
                    crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                    crate::consts::WRITE_TIMEOUT,
                )
                .await;

            server_write_part
                .h1_server_write_part
                .request_is_done(connection_id)
                .await;
            return;
        }

        server_write_part
            .h1_server_write_part
            .request_is_done(connection_id)
            .await;

        if web_socket_upgrade {
            let (remote_read_part, remote_loop_buffer) = remote_h1_reader.into_read_part();

            let (mut server_write_part, server_web_socket_data) = server_write_part
                .h1_server_write_part
                .upgrade_web_socket()
                .await;

            match server_web_socket_data.remote_connection.inner {
                RemoteConnectionInner::Http1Direct(inner) => {
                    tokio::spawn(crate::tcp_utils::copy_streams(
                        remote_read_part,
                        server_write_part,
                        remote_loop_buffer,
                        ssh_session_handler,
                    ));

                    tokio::spawn(crate::tcp_utils::copy_streams(
                        server_web_socket_data.read_part,
                        inner.remote_write_part,
                        server_web_socket_data.loop_buffer,
                        None,
                    ));
                }
                RemoteConnectionInner::Http1UnixSocket(inner) => {
                    tokio::spawn(crate::tcp_utils::copy_streams(
                        remote_read_part,
                        server_write_part,
                        remote_loop_buffer,
                        ssh_session_handler,
                    ));

                    tokio::spawn(crate::tcp_utils::copy_streams(
                        server_web_socket_data.read_part,
                        inner.remote_write_part,
                        server_web_socket_data.loop_buffer,
                        None,
                    ));
                }
                RemoteConnectionInner::Https1Direct(inner) => {
                    tokio::spawn(crate::tcp_utils::copy_streams(
                        remote_read_part,
                        server_write_part,
                        remote_loop_buffer,
                        ssh_session_handler,
                    ));

                    tokio::spawn(crate::tcp_utils::copy_streams(
                        server_web_socket_data.read_part,
                        inner.remote_write_part,
                        server_web_socket_data.loop_buffer,
                        None,
                    ));
                }
                RemoteConnectionInner::Http1OverSsh(inner) => {
                    tokio::spawn(crate::tcp_utils::copy_streams(
                        remote_read_part,
                        server_write_part,
                        remote_loop_buffer,
                        ssh_session_handler,
                    ));

                    tokio::spawn(crate::tcp_utils::copy_streams(
                        server_web_socket_data.read_part,
                        inner.remote_write_part,
                        server_web_socket_data.loop_buffer,
                        None,
                    ));
                }
                RemoteConnectionInner::Http1OverGateway(inner) => {
                    tokio::spawn(crate::tcp_utils::copy_streams(
                        remote_read_part,
                        server_write_part,
                        remote_loop_buffer,
                        ssh_session_handler,
                    ));
                    tokio::spawn(crate::tcp_utils::copy_streams(
                        server_web_socket_data.read_part,
                        inner.remote_write_part,
                        server_web_socket_data.loop_buffer,
                        None,
                    ));
                }
                RemoteConnectionInner::StaticContent { .. } => {
                    let _ = server_write_part
                        .write_to_socket(
                            crate::error_templates::ENDPOINT_CAN_NOT_BE_UPGRADED_TO_WEB_SOCKET
                                .as_slice(),
                        )
                        .await;
                    let _ = server_write_part.shutdown_socket().await;
                }
                RemoteConnectionInner::LocalFiles { .. } => {
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
