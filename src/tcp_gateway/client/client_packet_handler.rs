use std::sync::Arc;

use crate::tcp_gateway::{
    send_payload_to_gateway, TcpGatewayConnection, TcpGatewayContract, TcpGatewayForwardConnection,
    TcpGatewayPacketHandler,
};

use super::*;

pub struct TcpGatewayClientPacketHandler;

impl TcpGatewayClientPacketHandler {
    pub fn new() -> Self {
        Self
    }

    async fn handle_client_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        gateway_connection: &Arc<TcpGatewayClientConnection>,
    ) {
        match contract {
            TcpGatewayContract::Handshake { client_name } => {
                println!("Got handshake from gateway server {}", client_name);
            }
            TcpGatewayContract::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
                let remote_host = remote_host.to_string();
                let gateway_connection = gateway_connection.clone();
                tokio::spawn(super::packet_handlers::handle_forward_connect(
                    connection_id,
                    remote_host,
                    timeout,
                    gateway_connection,
                ));
            }
            TcpGatewayContract::Connected { connection_id } => {
                if let Some(connection) = gateway_connection
                    .get_forward_connection(connection_id)
                    .await
                {
                    println!(
                        "Somehow we got TcpGatewayContract::Connected Connection  with id {} to '{}' is connected. Can not happen on GatewayClient",
                        connection_id,
                        connection.get_addr()
                    );
                }
            }
            TcpGatewayContract::ConnectionError {
                connection_id,
                error,
            } => {
                if let Some(removed_connection) = gateway_connection
                    .remove_forward_connection(connection_id)
                    .await
                {
                    println!(
                        "Connection  with id {} to {} error: {}",
                        connection_id,
                        removed_connection.get_addr(),
                        error
                    );
                }
            }
            TcpGatewayContract::SendPayload {
                connection_id,
                payload,
            } => {
                send_payload_to_gateway(gateway_connection, connection_id, payload).await;
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
    type GateWayConnection = TcpGatewayClientConnection;
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        gateway_connection: &Arc<Self::GateWayConnection>,
    ) {
        self.handle_client_packet(contract, gateway_connection)
            .await;
    }
}
