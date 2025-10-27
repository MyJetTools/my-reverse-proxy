use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::{
    configurations::{HttpEndpointInfo, HttpListenPortConfiguration, ProxyPassLocationConfig},
    network_stream::*,
};

use super::*;

use crate::remote_connection::*;

use crate::h1_utils::*;

pub async fn serve_reverse_proxy<
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    TServerStream: NetworkStream<WritePart = WritePart, ReadPart = ReadPart> + Send + Sync + 'static,
>(
    server_stream: TServerStream,
    mut endpoint_info: Option<Arc<HttpEndpointInfo>>,
    listen_config: Arc<HttpListenPortConfiguration>,
) {
    let mut remote_connections: HashMap<i64, RemoteConnection> = HashMap::new();

    let mut h1_headers_builders = Http1HeadersBuilder::new();

    let (mut read_part, write_part) = server_stream.split();
    let mut loop_buffer = LoopBuffer::new();

    let write_part: Arc<Mutex<WritePart>> = Arc::new(Mutex::new(write_part));

    loop {
        let http_headers = read_headers(&mut read_part, &mut loop_buffer)
            .await
            .unwrap();

        if endpoint_info.is_none() {
            endpoint_info =
                try_find_endpoint_info(loop_buffer.get_data(), &http_headers, &listen_config);
        }

        let (location, end_point_info) = find_location(
            &http_headers,
            &mut endpoint_info,
            &listen_config,
            &mut loop_buffer,
        )
        .await;

        let connection = match remote_connections.get_mut(&location.id) {
            Some(connection) => connection,
            None => {
                let remote_connection = RemoteConnection::connect(
                    &location.proxy_pass_to,
                    &write_part,
                    end_point_info.clone(),
                )
                .await
                .unwrap();

                remote_connections.insert(location.id, remote_connection);
                remote_connections.get_mut(&location.id).unwrap()
            }
        };

        let content_length = http_headers.content_length;

        let host = location.proxy_pass_to.get_host();

        println!("{:?}", host);

        super::compile_headers(
            http_headers,
            &mut h1_headers_builders,
            &mut loop_buffer,
            &end_point_info.modify_request_headers,
            host,
        )
        .unwrap();

        connection
            .send_h1_header(&h1_headers_builders, crate::consts::WRITE_TIMEOUT)
            .await;

        super::transfer_body(&mut read_part, connection, content_length, &mut loop_buffer)
            .await
            .unwrap();
    }
}

async fn find_location<'s>(
    http_headers: &HttpHeaders,
    endpoint_info: &'s Option<Arc<HttpEndpointInfo>>,
    listen_config: &Arc<HttpListenPortConfiguration>,
    loop_buffer: &mut LoopBuffer,
) -> (&'s ProxyPassLocationConfig, &'s Arc<HttpEndpointInfo>) {
    let Some(endpoint_info) = &endpoint_info else {
        panic!("Can not find configuration for port {}", listen_config.port);
    };

    let first_line = http_headers.get_first_line(loop_buffer.get_data());
    let path = first_line.get_path();

    //println!("Path: '{}'", path);

    let Some(location) = endpoint_info.find_location(path) else {
        panic!("Location is not found for path {}", path);
    };

    (location, endpoint_info)
}

fn try_find_endpoint_info(
    buffer: &[u8],
    http_headers: &HttpHeaders,
    listen_config: &Arc<HttpListenPortConfiguration>,
) -> Option<Arc<HttpEndpointInfo>> {
    let Some(header_position) = http_headers.host_value.as_ref() else {
        return listen_config.get_http_endpoint_info(None);
    };

    let Ok(host) = std::str::from_utf8(&buffer[header_position.start..header_position.end]) else {
        return listen_config.get_http_endpoint_info(None);
    };

    match host.find(':') {
        Some(index) => listen_config.get_http_endpoint_info(Some(&host[..index])),
        None => listen_config.get_http_endpoint_info(Some(host)),
    }
}
