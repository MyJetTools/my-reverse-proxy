use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::tcp_gateway::*;

pub struct TcpGatewayServerPacketHandler;

#[async_trait::async_trait]
impl TcpGatewayPacketHandler for TcpGatewayServerPacketHandler {
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        gateway_connection: &Arc<TcpGatewayConnection>,
    ) {
        match contract {
            TcpGatewayContract::Handshake {
                client_name,
                timestamp,
            } => {
                let date_time = DateTimeAsMicroseconds::new(timestamp);
                println!(
                    "Got handshake from client. Timestamp: {}: {}",
                    client_name,
                    date_time.to_rfc3339()
                );
                gateway_connection.send_payload(&contract).await;
            }
            TcpGatewayContract::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
                println!(
                    "Connect to {}->{} with connection_id {} with timeout {:?}",
                    gateway_connection.gateway_id.as_str(),
                    remote_host,
                    connection_id,
                    timeout
                );
                let remote_addr = remote_host.to_string();
                let gateway_connection = gateway_connection.clone();

                tokio::spawn(crate::tcp_gateway::scripts::handle_forward_connect(
                    connection_id,
                    remote_addr,
                    timeout,
                    gateway_connection,
                ));
            }

            TcpGatewayContract::ForwardPayload {
                connection_id,
                payload,
            } => {
                crate::tcp_gateway::scripts::forward_payload(
                    gateway_connection,
                    connection_id,
                    payload,
                )
                .await
            }

            TcpGatewayContract::BackwardPayload {
                connection_id,
                payload,
            } => {
                gateway_connection
                    .notify_incoming_payload(connection_id, payload)
                    .await;
            }
            TcpGatewayContract::Ping => {
                gateway_connection
                    .send_payload(&TcpGatewayContract::Pong)
                    .await;
            }
            TcpGatewayContract::Pong => {}
            TcpGatewayContract::Connected { connection_id } => {
                gateway_connection
                    .notify_forward_proxy_connection_accepted(connection_id)
                    .await;
            }
            TcpGatewayContract::ConnectionError {
                connection_id,
                error,
            } => {
                println!("Got ConnectionError {}. Message: {}", connection_id, error);
                gateway_connection
                    .disconnect_forward_connection(connection_id)
                    .await;
            }
        }
    }
}
