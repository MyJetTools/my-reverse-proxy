use std::sync::Arc;

use super::*;

#[async_trait::async_trait]
pub trait TcpGatewayForwardConnection {
    fn get_addr(&self) -> &str;
    async fn send_payload(&self, payload: &[u8]) -> bool;
    async fn disconnect(&self);
}

pub async fn send_payload_to_gateway<
    TTcpGatewaySendPayload: TcpGatewayConnection + Send + Sync + 'static,
>(
    gateway_connection: &Arc<TTcpGatewaySendPayload>,
    connection_id: u32,
    payload: &[u8],
) {
    if let Some(connection) = gateway_connection
        .get_forward_connection(connection_id)
        .await
    {
        if !connection.send_payload(payload).await {
            gateway_connection
                .remove_forward_connection(connection_id)
                .await;
            let disconnected_payload = TcpGatewayContract::ConnectionError {
                connection_id,
                error: "Connection is closed",
            }
            .to_vec();
            gateway_connection
                .send_payload(disconnected_payload.as_slice())
                .await;
        }
    }
}
