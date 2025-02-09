use crate::tcp_gateway::{client::*, *};

pub async fn send_connection_error(
    gateway_connection: &TcpGatewayClientConnection,
    connection_id: u32,
    err: &str,
    is_error: bool,
) {
    if is_error {
        println!("{}", err);
    }

    let connection_fail = TcpGatewayContract::ConnectionError {
        connection_id: connection_id,
        error: err,
    };

    gateway_connection.send_payload(&connection_fail).await;
}
