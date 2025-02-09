use std::sync::Arc;

use x509_parser::nom::AsBytes;

use crate::tcp_gateway::*;

use super::TcpGatewayServerConnection;

pub struct TcpGatewayServerPacketHandler;

#[async_trait::async_trait]
impl TcpGatewayPacketHandler for TcpGatewayServerPacketHandler {
    type GateWayConnection = TcpGatewayServerConnection;
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        gateway_connection: &Arc<Self::GateWayConnection>,
    ) {
        match contract {
            TcpGatewayContract::Handshake { client_name } => {
                println!("Got handshake from client: {}", client_name);
                gateway_connection
                    .send_payload(contract.to_vec().as_bytes())
                    .await;
            }
            TcpGatewayContract::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
                let remote_addr = remote_host.to_string();
                let gateway_connection = gateway_connection.clone();

                tokio::spawn(
                    super::handle_connect_forward_endpoint::handle_connect_forward_endpoint(
                        connection_id,
                        remote_addr,
                        timeout,
                        gateway_connection,
                    ),
                );
            }

            TcpGatewayContract::SendPayload {
                connection_id,
                payload,
            } => {
                send_payload_to_gateway(gateway_connection, connection_id, payload).await;
            }
            TcpGatewayContract::Ping => {
                gateway_connection
                    .send_payload(TcpGatewayContract::PONG_PAYLOAD.as_slice())
                    .await;
            }
            TcpGatewayContract::Pong => {}
            TcpGatewayContract::Connected { connection_id: _ } => {}
            TcpGatewayContract::ConnectionError {
                connection_id,
                error,
            } => {
                if error.len() > 0 {
                    println!(
                        "Connection error for connection_id: {}. Error: {}",
                        connection_id, error
                    );
                }
                gateway_connection
                    .remove_forward_connection(connection_id)
                    .await;
            }
        }
    }
}
