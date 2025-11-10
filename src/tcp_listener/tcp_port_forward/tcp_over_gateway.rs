use std::{sync::Arc, time::Duration};

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{configurations::TcpEndpointHostConfig, types::AcceptedServerConnection};

pub async fn handle_connection(
    mut accepted_server_connection: AcceptedServerConnection,
    configuration: Arc<TcpEndpointHostConfig>,
    gateway_id: &Arc<String>,
    remote_endpoint: Arc<RemoteEndpointOwned>,
) {
    let connection_ip = accepted_server_connection.get_addr();
    if configuration.debug {
        println!(
            "Accepted connection forwarded to {}->{}",
            gateway_id.as_str(),
            remote_endpoint.as_str()
        );
    }

    let gateway_connection = crate::app::APP_CTX
        .get_gateway_by_id_with_next_connection_id(&gateway_id)
        .await;

    if gateway_connection.is_none() {
        if configuration.debug {
            println!(
                "Error connecting to remote tcp {} server. Gateway connection [{}] is not found. Closing incoming connection: {:?}",
                remote_endpoint.as_str(),
                gateway_id.as_str(),
                connection_ip
            );
        }
        let _ = accepted_server_connection.shutdown().await;
        return;
    }

    let (gateway_connection, connection_id) = gateway_connection.unwrap();

    let connection_result = gateway_connection
        .connect_to_forward_proxy_connection(
            remote_endpoint.clone(),
            Duration::from_secs(5),
            connection_id,
        )
        .await;

    if let Err(err) = &connection_result {
        if configuration.debug {
            println!(
                "Error connecting to remote tcp {} server: {:?}. Closing incoming connection: {:?}",
                remote_endpoint.as_str(),
                err,
                connection_ip
            );
        }
        let _ = accepted_server_connection.shutdown().await;
        return;
    }

    let proxy_connection = connection_result.unwrap();

    if configuration.debug {
        println!(
            "Accepted connection to {}->{}. Connection_id: {}",
            gateway_id,
            remote_endpoint.as_str(),
            connection_id,
        );
    }

    tokio::spawn(super::handle_port_forward(
        accepted_server_connection,
        proxy_connection,
        None,
    ));
}

/*
async fn copy_from_connection_to_gateway(
    mut server_read: impl NetworkStreamReadPart,
    mut proxy_write: impl NetworkStreamWritePart,
    listening_addr: SocketAddr,
    gateway_id: Arc<String>,
    connection_id: u32,
    remote_endpoint: Arc<RemoteEndpointOwned>,
    debug: bool,
) {
    let mut buffer = crate::tcp_utils::allocated_read_buffer(None);

    loop {
        let read_result = server_read
            .read_with_timeout(&mut buffer, crate::consts::READ_TIMEOUT)
            .await;

        let read_size = match read_result {
            Ok(size) => size,
            Err(err) => {
                let err = format!(
                    "Error reading from {}. Closing connection {} on gateway {}. Err: {:?}",
                    listening_addr,
                    connection_id,
                    gateway_id.as_str(),
                    err
                );

                if debug {
                    println!("{}", err);
                }

                let _ = proxy_write.shutdown().await;
                break;
            }
        };

        if read_size == 0 {
            let err = format!(
                "Reading from {} is closed. Closing connection {} on gateway {}",
                listening_addr,
                connection_id,
                gateway_id.as_str(),
            );

            if debug {
                println!("{}", err);
            }
            let _ = proxy_write.shutdown().await;
            break;
        }

        let result = proxy_write.write_all(&buffer[..read_size]).await;

        if let Err(err) = result {
            let err = format!(
                "Error writing to proxy connection {}-{} width id {}. Err: {:?}",
                gateway_id.as_str(),
                remote_endpoint.as_str(),
                connection_id,
                err
            );

            if debug {
                println!("{}", err);
            }
            let _ = proxy_write.shutdown().await;
            break;
        }
    }
}

async fn copy_from_gateway_to_connection(
    mut server_write: MyOwnedWriteHalf,
    mut proxy_read: ReadHalf<TcpGatewayProxyForwardStream>,
    listening_addr: SocketAddr,
    gateway_id: Arc<String>,
    connection_id: u32,
    remote_endpoint: Arc<RemoteEndpointOwned>,
    debug: bool,
) {
    let mut buffer = crate::tcp_utils::allocated_read_buffer(None);
    loop {
        let read_result = proxy_read.read(&mut buffer).await;

        let payload_size = match read_result {
            Ok(size) => size,
            Err(err) => {
                let err = format!(
                    "Error reading from gateway:{}->{} with connection id {}. Err: {}",
                    gateway_id.as_str(),
                    remote_endpoint.as_str(),
                    connection_id,
                    err
                );

                if debug {
                    println!("{}", err);
                }

                break;
            }
        };

        if payload_size == 0 {
            break;
        }

        let write_result = server_write
            .write_all_with_timeout(&buffer[..payload_size], Duration::from_secs(30))
            .await;

        if let Err(err) = write_result {
            let err = format!(
                "Write from gateway:{}->{} with connection id {} to {} is ended with error: `{:?}`. Closing connection",
                gateway_id.as_str(),
                remote_endpoint.as_str(),
                connection_id,
                listening_addr,
                err
            );

            if debug {
                println!("{}", err);
            }

            break;
        }
    }
}
 */
