use std::sync::Arc;

use rust_extensions::{file_utils::FilePath, SliceOrVec};

use crate::tcp_gateway::{GetFileStatus, TcpGatewayConnection};

pub async fn serve_file(
    request_id: u32,
    path: String,
    gateway_connection: Arc<TcpGatewayConnection>,
) {
    tokio::spawn(async move {
        let path = FilePath::from_str(path.as_str());

        let payload = match tokio::fs::read(path.as_str()).await {
            Ok(content) => {
                let response = crate::tcp_gateway::TcpGatewayContract::GetFileResponse {
                    request_id,
                    status: GetFileStatus::Ok,
                    content: SliceOrVec::AsVec(content),
                };

                response
            }

            Err(_) => {
                let response = crate::tcp_gateway::TcpGatewayContract::GetFileResponse {
                    request_id,
                    status: GetFileStatus::Error,
                    content: SliceOrVec::AsSlice(&[]),
                };

                response
            }
        };

        gateway_connection.send_payload(&payload).await;
    });
}
