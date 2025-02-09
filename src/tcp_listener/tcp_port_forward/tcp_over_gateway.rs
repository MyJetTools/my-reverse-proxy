use std::{net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::remote_endpoint::RemoteEndpointOwned;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};

use crate::{
    app::AppContext,
    configurations::TcpEndpointHostConfig,
    tcp_gateway::{forwarded_connection::TcpGatewayProxyForwardedConnection, TcpGatewayConnection},
    tcp_listener::AcceptedTcpConnection,
};

pub async fn handle_connection(
    app: Arc<AppContext>,
    mut accepted_server_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<TcpEndpointHostConfig>,
    gateway_id: Arc<String>,
    remote_host: Arc<RemoteEndpointOwned>,
) {
    if configuration.debug {
        println!(
            "Accepted connection forwarded to {}->{}",
            gateway_id.as_str(),
            remote_host.as_str()
        );
    }

    if let Some(ip_white_list_id) = configuration.ip_white_list_id.as_ref() {
        let ip_white_list = app
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
            let _ = accepted_server_connection.tcp_stream.shutdown().await;
            return;
        }
    }

    let gateway_client = app.gateway_clients.get(gateway_id.as_str());

    if gateway_client.is_none() {
        println!("Gateway connection with id '{}' is not found", gateway_id);
        let _ = accepted_server_connection.tcp_stream.shutdown().await;
        return;
    }

    let gateway_client = gateway_client.unwrap();

    if configuration.debug {
        println!("Connecting to {}", remote_host.as_str());
    }
    let connection_result = gateway_client
        .connect_to_forward_proxy_connection(remote_host.as_str())
        .await;

    if let Err(err) = &connection_result {
        if configuration.debug {
            println!(
                "Error connecting to remote tcp {} server: {:?}. Closing incoming connection: {}",
                remote_host.as_str(),
                err,
                accepted_server_connection.addr
            );
        }
        let _ = accepted_server_connection.tcp_stream.shutdown().await;
        return;
    }

    let (proxy_connection, gateway_connection) = connection_result.unwrap();

    if configuration.debug {
        println!(
            "Accepted connection to {}->{}. Connection_id: {}",
            gateway_id,
            proxy_connection.remote_endpoint.as_str(),
            proxy_connection.connection_id,
        );
    }

    let (read, write) = accepted_server_connection.tcp_stream.into_split();

    tokio::spawn(copy_from_connection_to_gateway(
        read,
        gateway_connection.clone(),
        proxy_connection.clone(),
        listening_addr,
        configuration.debug,
    ));
    tokio::spawn(copy_from_gateway_to_connection(
        write,
        gateway_connection,
        proxy_connection,
        listening_addr,
        configuration.debug,
    ));
}

async fn copy_from_connection_to_gateway(
    mut read: OwnedReadHalf,
    gateway_connection: Arc<TcpGatewayConnection>,
    proxy_connection: Arc<TcpGatewayProxyForwardedConnection>,
    listening_addr: SocketAddr,
    debug: bool,
) {
    let mut buffer = crate::tcp_utils::allocated_read_buffer();

    loop {
        let read_result = read.read(&mut buffer).await;

        let read_size = match read_result {
            Ok(size) => size,
            Err(err) => {
                let err = format!(
                    "Error reading from {}. Closing connection {} on gateway {}. Err: {:?}",
                    listening_addr,
                    proxy_connection.connection_id,
                    proxy_connection.get_gateway_id(),
                    err
                );

                if debug {
                    println!("{}", err);
                }

                gateway_connection
                    .disconnect_forward_proxy_connection(proxy_connection.connection_id, &err)
                    .await;
                break;
            }
        };

        if read_size == 0 {
            let err = format!(
                "Reading from {} is closed. Closing connection {} on gateway {}",
                listening_addr,
                proxy_connection.connection_id,
                proxy_connection.get_gateway_id(),
            );

            if debug {
                println!("{}", err);
            }
            gateway_connection
                .disconnect_forward_proxy_connection(proxy_connection.connection_id, &err)
                .await;
            break;
        }

        proxy_connection.send_payload(&buffer[..read_size]).await;
    }
}

async fn copy_from_gateway_to_connection(
    mut write: OwnedWriteHalf,
    gateway_connection: Arc<TcpGatewayConnection>,
    proxy_connection: Arc<TcpGatewayProxyForwardedConnection>,
    listening_addr: SocketAddr,
    debug: bool,
) {
    loop {
        let read_result = proxy_connection.receive_payload().await;

        let payload = match read_result {
            Ok(size) => size,
            Err(err) => {
                let err = format!(
                    "Error reading from gateway:{}->{} with connection id {}. Err: {}",
                    proxy_connection.get_gateway_id(),
                    proxy_connection.remote_endpoint,
                    proxy_connection.connection_id,
                    err
                );
                if debug {
                    println!("{}", err);
                }

                gateway_connection
                    .disconnect_forward_proxy_connection(proxy_connection.connection_id, &err)
                    .await;

                break;
            }
        };

        let write_future = write.write_all(payload.as_slice());

        let result = tokio::time::timeout(Duration::from_secs(30), write_future).await;

        if result.is_err() {
            let err = format!(
                "Write from gateway:{}->{} with connection id {} to {} is ended with timeout. Closing connection",
                proxy_connection.get_gateway_id(),
                proxy_connection.remote_endpoint,
                proxy_connection.connection_id,
                listening_addr
            );

            if debug {
                println!("{}", err);
            }
            gateway_connection
                .disconnect_forward_proxy_connection(proxy_connection.connection_id, &err)
                .await;
            break;
        }

        let result = result.unwrap();

        if let Err(err) = result {
            let err = format!(
                "Write from gateway:{}->{} with connection id {} to {} is ended with error: {:?}. Closing connection",
                proxy_connection.get_gateway_id(),
                proxy_connection.remote_endpoint,
                proxy_connection.connection_id,
                listening_addr,
                err
            );
            if debug {
                println!("{}", err);
            }
            gateway_connection
                .disconnect_forward_proxy_connection(proxy_connection.connection_id, &err)
                .await;
            break;
        }
    }
}
