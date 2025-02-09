use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::tcp_gateway::{TcpGatewayConnection, TcpGatewayContract, TcpGatewayPacketHandler};

pub struct TcpGatewayClientPacketHandler;

impl TcpGatewayClientPacketHandler {
    pub fn new() -> Self {
        Self
    }

    async fn handle_client_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        gateway_connection: &Arc<TcpGatewayConnection>,
    ) {
        match contract {
            TcpGatewayContract::Handshake {
                client_name,
                timestamp,
            } => {
                let timestamp = DateTimeAsMicroseconds::new(timestamp);
                println!(
                    "Got handshake from gateway server {} with timestamp {}",
                    client_name,
                    timestamp.to_rfc3339()
                );
            }
            TcpGatewayContract::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
                let remote_host = remote_host.to_string();
                let gateway_connection = gateway_connection.clone();
                tokio::spawn(crate::tcp_gateway::scripts::handle_forward_connect(
                    connection_id,
                    remote_host,
                    timeout,
                    gateway_connection,
                ));
            }
            TcpGatewayContract::Connected { connection_id } => {
                println!("Got Gateway payload connected: {}", connection_id);
                gateway_connection
                    .notify_proxy_connection_accepted(connection_id)
                    .await;
            }
            TcpGatewayContract::ConnectionError {
                connection_id,
                error,
            } => {
                gateway_connection
                    .notify_proxy_connection_disconnected(connection_id, error)
                    .await;
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
                println!(
                    "Got BackwardPayload for connection_id: {}. Size: {}",
                    connection_id,
                    payload.len()
                );
                gateway_connection
                    .notify_incoming_payload(connection_id, payload)
                    .await;
            }
            TcpGatewayContract::Ping => {}
            TcpGatewayContract::Pong => {
                println!("Got PONG")
            }
        }
    }
}

#[async_trait::async_trait]
impl TcpGatewayPacketHandler for TcpGatewayClientPacketHandler {
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        gateway_connection: &Arc<TcpGatewayConnection>,
    ) {
        self.handle_client_packet(contract, gateway_connection)
            .await;
    }
}
