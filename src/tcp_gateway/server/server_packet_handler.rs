use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::tcp_gateway::*;

pub struct TcpGatewayServerPacketHandler {
    debug: bool,
}

impl TcpGatewayServerPacketHandler {
    pub fn new(debug: bool) -> Self {
        Self { debug }
    }
}

#[async_trait::async_trait]
impl TcpGatewayPacketHandler for TcpGatewayServerPacketHandler {
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        tcp_gateway: &Arc<TcpGatewayInner>,
        gateway_connection: &Arc<TcpGatewayConnection>,
    ) -> Result<(), String> {
        match contract {
            TcpGatewayContract::Handshake {
                gateway_name,
                timestamp,
            } => {
                let timestamp = DateTimeAsMicroseconds::new(timestamp);

                println!(
                    "Got handshake from gateway {}. Timestamp: {}",
                    gateway_name,
                    timestamp.to_rfc3339()
                );

                let now = DateTimeAsMicroseconds::now();

                let loading_packet = now - timestamp;
                if loading_packet.get_full_seconds() > 5 {
                    return Err(format!("Handshake packet is too old. {:?}", loading_packet));
                }

                gateway_connection.set_gateway_id(gateway_name).await;
                tcp_gateway
                    .set_gateway_connection(gateway_name, gateway_connection.clone().into())
                    .await;
                gateway_connection.send_payload(&contract).await;
            }
            TcpGatewayContract::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
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
                if self.debug {
                    println!("Got ConnectionError {}. Message: {}", connection_id, error);
                }

                gateway_connection
                    .disconnect_forward_proxy_connection(connection_id, error)
                    .await;
            }
        }
        Ok(())
    }
}
