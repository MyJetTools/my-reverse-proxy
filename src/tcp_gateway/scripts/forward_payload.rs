use crate::tcp_gateway::TcpGatewayConnection;

pub async fn forward_payload(
    gateway_connection: &TcpGatewayConnection,
    connection_id: u32,
    payload: &[u8],
) {
    if let Some(forward_connection) = gateway_connection
        .get_forward_connection(connection_id)
        .await
    {
        if !forward_connection.send_payload(payload).await {
            gateway_connection
                .disconnect_forward_connection(connection_id)
                .await;
        }
    }
}
