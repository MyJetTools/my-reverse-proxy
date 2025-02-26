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
                support_compression: _,
                timestamp,
            } => {
                let timestamp = DateTimeAsMicroseconds::new(timestamp);
                gateway_connection.set_connection_timestamp(timestamp);
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
                if !gateway_connection.is_incoming_forward_connection_allowed() {
                    crate::tcp_gateway::scripts::send_connection_error(
                        gateway_connection.as_ref(),
                        connection_id,
                        "Forward connections are disabled this way to gateway",
                        true,
                        false,
                    )
                    .await;
                    return;
                }

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
                println!(
                    "Gateway: [{}]. Connection error with id {}. Message: {}",
                    gateway_connection.get_gateway_id().await,
                    connection_id,
                    error
                );

                gateway_connection
                    .disconnect_forward_proxy_connection(connection_id, error)
                    .await;
            }
            TcpGatewayContract::ForwardPayload {
                connection_id,

                payload,
            } => {
                crate::tcp_gateway::scripts::forward_payload(
                    gateway_connection,
                    connection_id,
                    payload.as_slice(),
                )
                .await
            }

            TcpGatewayContract::BackwardPayload {
                connection_id,
                payload,
            } => {
                gateway_connection
                    .incoming_payload_for_proxy_connection(connection_id, payload.as_slice())
                    .await;
            }
            TcpGatewayContract::Ping => {}
            TcpGatewayContract::Pong => {
                gateway_connection.ping_stop_watch.pause();
                let duration = gateway_connection
                    .ping_stop_watch
                    .duration()
                    .as_positive_or_zero();

                gateway_connection.last_ping_duration.update(duration);
                let update_ping_time = TcpGatewayContract::UpdatePingTime { duration };
                gateway_connection.send_payload(&update_ping_time).await;
            }
            TcpGatewayContract::GetFileRequest { path, request_id } => {
                crate::tcp_gateway::scripts::serve_file(
                    request_id,
                    path.to_string(),
                    gateway_connection.clone(),
                )
                .await
            }

            TcpGatewayContract::GetFileResponse {
                request_id,
                status,
                content,
            } => {
                gateway_connection
                    .notify_file_response(request_id, status, content)
                    .await;
            }

            TcpGatewayContract::UpdatePingTime { duration: _ } => {}
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
