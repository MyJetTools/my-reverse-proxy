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

        println!("Loading File {}", path.as_str());

        let payload = match tokio::fs::read(path.as_str()).await {
            Ok(content) => {
                println!("Got File response len: {}", content.len());
                crate::tcp_gateway::TcpGatewayContract::GetFileResponse {
                    request_id,
                    status: GetFileStatus::Ok,
                    content: SliceOrVec::AsVec(content),
                }
            }

            Err(err) => {
                println!("Got File response len: {:?}", err);
                crate::tcp_gateway::TcpGatewayContract::GetFileResponse {
                    request_id,
                    status: GetFileStatus::Error,
                    content: SliceOrVec::AsSlice(&[]),
                }
            }
        };

        gateway_connection.send_payload(&payload).await;
    });
}
