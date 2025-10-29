use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::network_stream::*;

use super::*;

use crate::remote_connection::h1::*;

pub struct CurrentRequest {
    pub request_id: u64,
    pub buffer: Vec<u8>,
    pub done: bool,
}

impl CurrentRequest {
    pub fn new(request_id: u64) -> Self {
        Self {
            request_id,
            buffer: vec![],
            done: false,
        }
    }
}

pub struct HttpServerSingleThreadedPart<WritePart: NetworkStreamWritePart + Send + Sync + 'static> {
    pub server_write_part: WritePart,
    pub current_requests: Vec<CurrentRequest>,
}

pub async fn serve_reverse_proxy<
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    TServerStream: NetworkStream<WritePart = WritePart, ReadPart = ReadPart> + Send + Sync + 'static,
>(
    server_stream: TServerStream,
    mut http_connection_info: HttpConnectionInfo,
) {
    let mut remote_connections: HashMap<i64, RemoteConnection> = HashMap::new();

    let (read_part, server_write_part) = server_stream.split();

    let mut h1_read_part = H1ReadPart::new(read_part);

    let server_single_threaded_part: Arc<Mutex<HttpServerSingleThreadedPart<WritePart>>> =
        Arc::new(Mutex::new(HttpServerSingleThreadedPart {
            server_write_part,
            current_requests: Vec::new(),
        }));

    let mut request_id = 0;

    loop {
        request_id += 1;
        if let Err(err) = execute_request(
            request_id,
            &mut http_connection_info,
            &mut h1_read_part,
            &mut remote_connections,
            &server_single_threaded_part,
        )
        .await
        {
            println!("Response: {:?}", err);
            let content = match &err {
                ProxyServerError::NetworkError(network_error) => {
                    println!("Http Server connections network error. {:?}", network_error);
                    break;
                }
                ProxyServerError::ParsingPayloadError(_) => {
                    crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice()
                }
                ProxyServerError::BufferAllocationFail => {
                    crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
                }
                ProxyServerError::ChunkHeaderParseError => {
                    crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice()
                }
                ProxyServerError::HeadersParseError(_) => {
                    crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice()
                }
                ProxyServerError::CanNotConnectToRemoteResource(_) => {
                    crate::error_templates::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
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
            server_single_threaded_part
                .lock()
                .await
                .server_write_part
                .write_http_payload(request_id, content, crate::consts::WRITE_TIMEOUT)
                .await
                .unwrap();
        }
    }

    println!(
        "Server http connection {} is closed",
        http_connection_info.listening_addr
    )
}

async fn execute_request<
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
>(
    request_id: u64,
    http_connection_info: &mut HttpConnectionInfo,
    h1_read_part: &mut H1ReadPart<ReadPart>,
    remote_connections: &mut HashMap<i64, RemoteConnection>,
    server_single_threaded_part: &Arc<Mutex<HttpServerSingleThreadedPart<WritePart>>>,
) -> Result<(), ProxyServerError> {
    let request_headers = h1_read_part.read_headers().await?;

    if http_connection_info.endpoint_info.is_none() {
        http_connection_info.endpoint_info = h1_read_part
            .try_find_endpoint_info(&request_headers, &http_connection_info.listen_config);
    }

    let (location, end_point_info) = h1_read_part
        .find_location(&request_headers, &http_connection_info)
        .await?;

    let identity = h1_read_part
        .authorize(end_point_info, &http_connection_info, &request_headers)
        .await?;

    if !end_point_info.user_is_allowed(&identity).await {
        return Err(ProxyServerError::NotAuthorized);
    }

    let mut connection = match remote_connections.get_mut(&location.id) {
        Some(connection) => connection,
        None => {
            let remote_connection = RemoteConnection::connect(&location.proxy_pass_to).await;

            match remote_connection {
                Ok(remote_connection) => {
                    remote_connections.insert(location.id, remote_connection);
                    remote_connections.get_mut(&location.id).unwrap()
                }
                Err(err) => return Err(ProxyServerError::CanNotConnectToRemoteResource(err)),
            }
        }
    };

    let content_length = request_headers.content_length;

    h1_read_part
        .compile_headers(
            request_headers,
            &end_point_info.modify_request_headers,
            &http_connection_info,
            &identity,
        )
        .unwrap();

    let send_headers_result = connection
        .send_h1_header(
            request_id,
            &h1_read_part.h1_headers_builder,
            crate::consts::WRITE_TIMEOUT,
        )
        .await;

    if !send_headers_result {
        remote_connections.remove(&location.id);

        let remote_connection = RemoteConnection::connect(&location.proxy_pass_to).await;

        match remote_connection {
            Ok(remote_connection) => {
                remote_connections.insert(location.id, remote_connection);
                connection = remote_connections.get_mut(&location.id).unwrap();
            }
            Err(err) => return Err(ProxyServerError::CanNotConnectToRemoteResource(err)),
        }

        let send_headers_result = connection
            .send_h1_header(
                request_id,
                &h1_read_part.h1_headers_builder,
                crate::consts::WRITE_TIMEOUT,
            )
            .await;

        if !send_headers_result {
            return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
                NetworkError::Other("Remote resource is disconnected"),
            ));
        }
    }

    h1_read_part
        .transfer_body(request_id, connection, content_length)
        .await
        .unwrap();

    server_single_threaded_part
        .lock()
        .await
        .current_requests
        .push(CurrentRequest::new(request_id));

    //let server_single_threaded_part: Arc<Mutex<HttpServerSingleThreadedPart<WritePart>>> =
    //    server_single_threaded_part.clone();

    let connected = connection.read_http_response(Http1ConnectionContext {
        server_single_threaded_part: server_single_threaded_part.clone(),
        http_connection_info: http_connection_info.clone(),
        end_point_info: end_point_info.clone(),
        request_id: request_id,
    });

    println!("Connections: {}", remote_connections.len());

    if !connected {
        return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
            NetworkError::Other("Remote connection is lost"),
        ));
    }

    Ok(())
}
