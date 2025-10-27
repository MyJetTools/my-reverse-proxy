use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
    configurations::HttpEndpointInfo, h1_server::LoopBuffer, h1_utils::Http1HeadersBuilder,
    network_stream::*,
};

pub async fn from_remote_to_server_loop<
    ServerWritePart: NetworkStreamWritePart + Send + Sync + 'static,
    RemoteReadPart: NetworkStreamReadPart + Send + Sync + 'static,
>(
    server_write_part: Arc<Mutex<ServerWritePart>>,
    end_point_info: Arc<HttpEndpointInfo>,
    mut remote_read_part: RemoteReadPart,
) {
    let mut loop_buffer = LoopBuffer::new();

    let mut h1_headers_builder = Http1HeadersBuilder::new();

    loop {
        let http_headers = super::read_headers(&mut remote_read_part, &mut loop_buffer)
            .await
            .unwrap();

        let mut write_access = server_write_part.lock().await;

        let content_length = http_headers.content_length;

        super::compile_headers(
            http_headers,
            &mut h1_headers_builder,
            &mut loop_buffer,
            &end_point_info.modify_response_headers,
            None,
        )
        .unwrap();

        write_access
            .write_to_socket(h1_headers_builder.as_slice())
            .await
            .unwrap();

        super::transfer_body(
            &mut remote_read_part,
            &mut *write_access,
            content_length,
            &mut loop_buffer,
        )
        .await
        .unwrap();
    }
}
