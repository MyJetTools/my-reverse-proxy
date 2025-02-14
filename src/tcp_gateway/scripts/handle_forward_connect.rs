use std::{sync::Arc, time::Duration};


use crate::tcp_gateway::{ forwarded_connection::TcpGatewayForwardConnection, *};

pub async fn handle_forward_connect(
    connection_id: u32,
    remote_host: String,
    timeout: Duration,
    gateway_connection: Arc<TcpGatewayConnection>,
) {
    if gateway_connection
        .has_forward_connection(connection_id)
        .await
    {

        let err = 
        format!("Attempt to establish client forward connection is fail. ConnectionId {} is already has a connection", connection_id);

        crate::tcp_gateway::scripts::send_connection_error(gateway_connection.as_ref(), connection_id, err.as_str(), true, true).await;
        return;
    }

    let  connection_result = TcpGatewayForwardConnection::connect(
        connection_id,
        gateway_connection.clone(),
        Arc::new(remote_host.to_string()),
        timeout,
        gateway_connection.get_aes_key().clone()
    ).await;

    match connection_result{
        Ok(forward_connection) => {
            let connected_payload = TcpGatewayContract::Connected { connection_id } ;
        gateway_connection.send_payload(&connected_payload).await;

        let forward_connection = Arc::new(forward_connection);

        gateway_connection.add_forward_connection(connection_id, forward_connection).await;
  
        },
        Err(err) => {
            crate::tcp_gateway::scripts::send_connection_error(gateway_connection.as_ref(), connection_id, err.as_str(), true,true).await;
        },
    }
}
