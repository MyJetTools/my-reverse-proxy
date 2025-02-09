use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::tcp_gateway::{
    TcpGatewayConnection, TcpGatewayContract, TcpGatewayInner, TcpGatewayPacketHandler,
};

pub struct TcpGatewayClientPacketHandler {
    debug: bool,
}

impl TcpGatewayClientPacketHandler {
    pub fn new(debug: bool) -> Self {
        Self { debug }
    }

    async fn handle_client_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        tcp_gateway: &Arc<TcpGatewayInner>,
        gateway_connection: &Arc<TcpGatewayConnection>,
    ) {
        match contract {
            TcpGatewayContract::Handshake {
                gateway_name,
                timestamp,
            } => {
                let timestamp = DateTimeAsMicroseconds::new(timestamp);

                println!(
                    "Got handshake confirm from server gateway with id {} and timestamp {}",
                    gateway_name,
                    timestamp.to_rfc3339()
                );

                gateway_connection.set_gateway_id(gateway_name).await;
                tcp_gateway
                    .set_gateway_connection(gateway_name, gateway_connection.clone().into())
                    .await;
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
                if self.debug {
                    println!("Got Gateway payload connected: {}", connection_id);
                }

                gateway_connection
                    .notify_forward_proxy_connection_accepted(connection_id)
                    .await;
            }
            TcpGatewayContract::ConnectionError {
                connection_id,
                error,
            } => {
                if self.debug {
                    println!("Got ConnectionError {}. Message: {}", connection_id, error);
                }

                gateway_connection
                    .disconnect_forward_proxy_connection(connection_id, error)
                    .await;
            }
            TcpGatewayContract::ForwardPayload {
                connection_id,
                payload,
            } => {
                println!("Forward_Payload: {}", payload.len());
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
                println!("Backward_Payload: {}", payload.len());
                gateway_connection
                    .notify_incoming_payload(connection_id, payload)
                    .await;
            }
            TcpGatewayContract::Ping => {}
            TcpGatewayContract::Pong => {}
        }
    }
}

#[async_trait::async_trait]
impl TcpGatewayPacketHandler for TcpGatewayClientPacketHandler {
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        tcp_gateway: &Arc<TcpGatewayInner>,
        gateway_connection: &Arc<TcpGatewayConnection>,
    ) -> Result<(), String> {
        self.handle_client_packet(contract, tcp_gateway, gateway_connection)
            .await;

        Ok(())
    }
}
