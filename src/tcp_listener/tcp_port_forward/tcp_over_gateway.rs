use std::{net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::remote_endpoint::RemoteEndpointOwned;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tokio::io::{ReadHalf, WriteHalf};

use crate::tcp_or_unix::NetworkStreamWritePart;
use crate::{
    configurations::TcpEndpointHostConfig,
    tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
    tcp_listener::AcceptedTcpConnection,
    tcp_or_unix::{MyOwnedReadHalf, MyOwnedWriteHalf},
};

pub async fn handle_connection(
    mut accepted_server_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<TcpEndpointHostConfig>,
    gateway_id: Arc<String>,
    remote_endpoint: Arc<RemoteEndpointOwned>,
) {
    if configuration.debug {
        println!(
            "Accepted connection forwarded to {}->{}",
            gateway_id.as_str(),
            remote_endpoint.as_str()
        );
    }

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

    let gateway_connection = crate::app::APP_CTX
        .get_gateway_by_id_with_next_connection_id(&gateway_id)
        .await;

    if gateway_connection.is_none() {
        if configuration.debug {
            println!(
                "Error connecting to remote tcp {} server. Gateway connection [{}] is not found. Closing incoming connection: {}",
                remote_endpoint.as_str(),
                gateway_id.as_str(),
                accepted_server_connection.addr
            );
        }
        let _ = accepted_server_connection.network_stream.shutdown().await;
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
                "Error connecting to remote tcp {} server: {:?}. Closing incoming connection: {}",
                remote_endpoint.as_str(),
                err,
                accepted_server_connection.addr
            );
        }
        let _ = accepted_server_connection.network_stream.shutdown().await;
        return;
    }

    let proxy_connection = connection_result.unwrap();

    let (proxy_read, proxy_write) = tokio::io::split(proxy_connection);

    if configuration.debug {
        println!(
            "Accepted connection to {}->{}. Connection_id: {}",
            gateway_id,
            remote_endpoint.as_str(),
            connection_id,
        );
    }

    let (server_read, server_write) = accepted_server_connection.network_stream.into_split();

    tokio::spawn(copy_from_connection_to_gateway(
        server_read,
        proxy_write,
        listening_addr,
        gateway_id.clone(),
        connection_id,
        remote_endpoint.clone(),
        configuration.debug,
    ));
    tokio::spawn(copy_from_gateway_to_connection(
        server_write,
        proxy_read,
        listening_addr,
        gateway_id,
        connection_id,
        remote_endpoint,
        configuration.debug,
    ));
}

async fn copy_from_connection_to_gateway(
    mut server_read: MyOwnedReadHalf,
    mut proxy_write: WriteHalf<TcpGatewayProxyForwardStream>,
    listening_addr: SocketAddr,
    gateway_id: Arc<String>,
    connection_id: u32,
    remote_endpoint: Arc<RemoteEndpointOwned>,
    debug: bool,
) {
    let mut buffer = crate::tcp_utils::allocated_read_buffer(None);

    loop {
        let read_result = server_read.read(&mut buffer).await;

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
                "Write from gateway:{}->{} with connection id {} to {} is ended with error: `{}`. Closing connection",
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
