use crate::tcp_gateway::*;

pub async fn send_connection_error(
    gateway_connection: &TcpGatewayConnection,
    connection_id: u32,
    err: &str,
    send_force: bool,
    is_error: bool,
) {
    if is_error {
        let gateway_id = gateway_connection.get_gateway_id().await;
        println!("Gateway:[{}] {}", gateway_id.as_str(), err);
    }

    let removed = gateway_connection
        .remove_forward_connection(connection_id)
        .await
        .is_some();

    if send_force || removed {
        let connection_fail = TcpGatewayContract::ConnectionError {
            connection_id: connection_id,
            error: err,
        };

        gateway_connection.send_payload(&connection_fail).await;
    }
}
