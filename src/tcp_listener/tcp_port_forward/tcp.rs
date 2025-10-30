use std::{net::SocketAddr, sync::Arc};

use rust_extensions::remote_endpoint::RemoteEndpointOwned;
use tokio::net::TcpStream;

use crate::{configurations::*, tcp_listener::AcceptedTcpConnection};

pub async fn handle_connection(
    mut accepted_server_connection: AcceptedTcpConnection,
    _listening_addr: SocketAddr,
    configuration: Arc<TcpEndpointHostConfig>,
    remote_host: Arc<RemoteEndpointOwned>,
) {
    if let Some(ip_white_list_id) = configuration.ip_white_list_id.as_ref() {
        let ip_white_list = crate::app::APP_CTX
            .current_configuration
            .get(|config| config.white_list_ip_list.get(ip_white_list_id))
            .await;

        let mut shut_down_connection = false;

        match ip_white_list {
            Some(white_list_ip) => {
                if !white_list_ip.is_whitelisted(&accepted_server_connection.addr.ip()) {
                    shut_down_connection = true;
                    if configuration.debug {
                        println!(
                            "Incoming connection from {} is not whitelisted. Closing it",
                            accepted_server_connection.addr
                        );
                    }
                }
            }
            None => {
                shut_down_connection = true;
                if configuration.debug {
                    println!(
                        "Incoming connection from {} has invalid white_list_id {ip_white_list_id}. Closing it",
                        accepted_server_connection.addr
                    );
                }
            }
        }
        if shut_down_connection {
            let _ = accepted_server_connection.network_stream.shutdown().await;
            return;
        }
    }

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
                "Timeout while connecting to remote tcp {} server. Closing incoming connection: {}",
                remote_host.as_str(),
                accepted_server_connection.addr
            );
        }
        let _ = accepted_server_connection.network_stream.shutdown().await;
        return;
    }

    let remote_tcp_connection_result = remote_tcp_connection_result.unwrap();

    let remote_tcp_connection_result = match remote_tcp_connection_result {
        Ok(value) => value,
        Err(err) => {
            if configuration.debug {
                println!(
                "Error connecting to remote tcp {} server: {:?}. Closing incoming connection: {}",
                remote_host.as_str(),
                err,
                accepted_server_connection.addr
            );
            }
            let _ = accepted_server_connection.network_stream.shutdown().await;
            return;
        }
    };

    tokio::spawn(super::handle_port_forward(
        accepted_server_connection,
        remote_tcp_connection_result,
        None,
    ));
}
