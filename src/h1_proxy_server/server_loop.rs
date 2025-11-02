use std::collections::HashMap;

use crate::network_stream::*;

use super::*;

use crate::h1_remote_connection::*;

pub async fn serve_reverse_proxy<
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    TServerStream: NetworkStream<WritePart = WritePart, ReadPart = ReadPart> + Send + Sync + 'static,
>(
    server_stream: TServerStream,
    mut http_connection_info: HttpConnectionInfo,
) {
    let mut remote_connections: HashMap<i64, RemoteConnection> = HashMap::new();

    let (server_read_part, server_write_part) = server_stream.split();

    let timeouts = crate::types::HttpTimeouts {
        read_timeout: crate::consts::READ_TIMEOUT,
        write_timeout: crate::consts::WRITE_TIMEOUT,
        connect_timeout: crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
    };

    let mut h1_reader = H1Reader::new(server_read_part, timeouts);

    let h1_server_write_part = H1ServerWritePart::new(server_write_part);

    let mut request_id = 0;

    loop {
        request_id += 1;
        let execute_request_result = execute_request(
            &mut http_connection_info,
            &mut h1_reader,
            &mut remote_connections,
            &h1_server_write_part,
        )
        .await;

        match execute_request_result {
            Ok(web_socket_upgrade) => {
                if let Some(web_socket_upgrade) = web_socket_upgrade {
                    if let Some(connection) =
                        remote_connections.remove(&web_socket_upgrade.location_id)
                    {
                        let (server_read_part, loop_buffer) = h1_reader.into_read_part();

                        h1_server_write_part
                            .add_web_socket_upgrade(connection, server_read_part, loop_buffer)
                            .await;

                        return;
                    }
                }
            }
            Err(err) => {
                if err.can_be_printed_as_debug() {
                    println!("Response Err: {:?}", err);
                }

                let content = match &err {
                    ProxyServerError::NetworkError(network_error) => {
                        if !network_error.is_timeout() {
                            println!("Http Server connections network error. {:?}", network_error);
                        }

                        break;
                    }
                    ProxyServerError::ParsingPayloadError(_) => {
                        crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE
                            .as_slice()
                    }
                    ProxyServerError::BufferAllocationFail => {
                        println!("Buffer allocation fail - server loop");
                        crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
                    }
                    ProxyServerError::ChunkHeaderParseError => {
                        crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE
                            .as_slice()
                    }
                    ProxyServerError::HeadersParseError(_) => {
                        crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE
                            .as_slice()
                    }
                    ProxyServerError::CanNotConnectToRemoteResource(err) => {
                        if err.as_timeout().is_some() {
                            crate::error_templates::ERROR_TIMEOUT.as_slice()
                        } else {
                            crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
                        }
                    }
                    ProxyServerError::LocationIsNotFound => {
                        crate::error_templates::LOCATION_IS_NOT_FOUND.as_slice()
                    }
                    ProxyServerError::HttpConfigurationIsNotFound => {
                        crate::error_templates::CONFIGURATION_IS_NOT_FOUND.as_slice()
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
                h1_server_write_part
                    .write_http_payload_with_timeout(
                        request_id,
                        content,
                        crate::consts::WRITE_TIMEOUT,
                    )
                    .await
                    .unwrap();
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
    remote_connections: &mut HashMap<i64, RemoteConnection>,
    h1_server_write_part: &H1ServerWritePart<WritePart, ReadPart>,
) -> Result<Option<WebSocketUpgradeResult>, ProxyServerError> {
    let request_headers = h1_reader.read_headers().await?;

    if http_connection_info.endpoint_info.is_none() {
        http_connection_info.endpoint_info =
            h1_reader.try_find_endpoint_info(&request_headers, &http_connection_info.listen_config);
    }

    let (location, end_point_info) = h1_reader
        .find_location(&request_headers, &http_connection_info)
        .await?;

    let identity = h1_reader
        .authorize(end_point_info, &http_connection_info, &request_headers)
        .await?;

    if !end_point_info.user_is_allowed(&identity).await {
        return Err(ProxyServerError::NotAuthorized);
    }

    let http_connection_context = Http1ServerConnectionContext {
        h1_server_write_part: h1_server_write_part.clone(),
        http_connection_info: http_connection_info.clone(),
        end_point_info: end_point_info.clone(),
    };

    let mut connection = match remote_connections.get_mut(&location.id) {
        Some(connection) => connection,
        None => {
            let remote_connection =
                RemoteConnection::connect(&location.proxy_pass_to, &http_connection_context).await;

            match remote_connection {
                Ok(remote_connection) => {
                    println!("Connected to remote source");
                    remote_connections.insert(location.id, remote_connection);
                    remote_connections.get_mut(&location.id).unwrap()
                }
                Err(err) => return Err(ProxyServerError::CanNotConnectToRemoteResource(err)),
            }
        }
    };

    let content_length = request_headers.content_length;

    let web_socket_upgrade = h1_reader.compile_headers(
        request_headers,
        &end_point_info.modify_request_headers,
        &http_connection_info,
        &identity,
        connection.mcp_path.as_deref(),
    )?;

    let send_headers_result = connection
        .send_h1_header(&h1_reader.h1_headers_builder, crate::consts::WRITE_TIMEOUT)
        .await;

    if !send_headers_result {
        remote_connections.remove(&location.id);

        println!("Doing reconnection to remote connection");

        let remote_connection =
            RemoteConnection::connect(&location.proxy_pass_to, &http_connection_context).await;

        match remote_connection {
            Ok(remote_connection) => {
                remote_connections.insert(location.id, remote_connection);
                connection = remote_connections.get_mut(&location.id).unwrap();
            }
            Err(err) => return Err(ProxyServerError::CanNotConnectToRemoteResource(err)),
        }

        let send_headers_result = connection
            .send_h1_header(&h1_reader.h1_headers_builder, crate::consts::WRITE_TIMEOUT)
            .await;

        if !send_headers_result {
            return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
                NetworkError::OtherStr("Remote resource is disconnected"),
            ));
        }
    }

    if web_socket_upgrade {
        return Ok(Some(WebSocketUpgradeResult {
            location_id: location.id,
        }));
    }

    h1_reader
        .transfer_body(connection.connection_id, connection, content_length)
        .await?;

    h1_server_write_part
        .add_current_request(connection.connection_id)
        .await;

    //let server_single_threaded_part: Arc<Mutex<HttpServerSingleThreadedPart<WritePart>>> =
    //    server_single_threaded_part.clone();

    let connected = connection.read_http_response(http_connection_context);

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
