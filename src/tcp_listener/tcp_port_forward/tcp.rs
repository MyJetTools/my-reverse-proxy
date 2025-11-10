use std::sync::Arc;

use rust_extensions::remote_endpoint::RemoteEndpointOwned;
use tokio::net::TcpStream;

use crate::{configurations::*, types::AcceptedServerConnection};

pub async fn handle_connection(
    mut accepted_server_connection: AcceptedServerConnection,

    configuration: Arc<TcpEndpointHostConfig>,
    remote_host: Arc<RemoteEndpointOwned>,
) {
    let socket_addr = accepted_server_connection.get_addr();

    let remote_tcp_connection_result = tokio::time::timeout(
        crate::app::APP_CTX
            .connection_settings
            .remote_connect_timeout,
        TcpStream::connect(remote_host.as_str()),
    )
    .await;

    if remote_tcp_connection_result.is_err() {
        if configuration.debug {
            println!(
                "Timeout while connecting to remote tcp {} server. Closing incoming connection: {:?}",
                remote_host.as_str(),
                socket_addr
            );
        }
        let _ = accepted_server_connection.shutdown().await;
        return;
    }

    let remote_tcp_connection_result = remote_tcp_connection_result.unwrap();

    let remote_tcp_connection_result = match remote_tcp_connection_result {
        Ok(value) => value,
        Err(err) => {
            if configuration.debug {
                println!(
                "Error connecting to remote tcp {} server: {:?}. Closing incoming connection: {:?}",
                remote_host.as_str(),
                err,
                socket_addr
            );
            }
            let _ = accepted_server_connection.shutdown().await;
            return;
        }
    };

    tokio::spawn(super::handle_port_forward(
        accepted_server_connection,
        remote_tcp_connection_result,
        None,
    ));
}
