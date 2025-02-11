use std::{net::SocketAddr, sync::Arc, time::Duration};

use rust_extensions::remote_endpoint::RemoteEndpointOwned;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};

use crate::{
    configurations::TcpEndpointHostConfig,
    tcp_gateway::forwarded_connection::{ProxyConnectionReadHalf, ProxyConnectionWriteHalf},
    tcp_listener::AcceptedTcpConnection,
};

pub async fn handle_connection(
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
            let _ = accepted_server_connection.tcp_stream.shutdown().await;
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
                remote_host.as_str(),
                gateway_id.as_str(),
                accepted_server_connection.addr
            );
        }
        let _ = accepted_server_connection.tcp_stream.shutdown().await;
        return;
    }

    let (gateway_connection, connection_id) = gateway_connection.unwrap();

    let connection_result = gateway_connection
        .connect_to_forward_proxy_connection(
            remote_host.as_str(),
            Duration::from_secs(5),
            connection_id,
        )
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

    let (proxy_read, proxy_write) = connection_result.unwrap();

    if configuration.debug {
        println!(
            "Accepted connection to {}->{}. Connection_id: {}",
            gateway_id,
            remote_host.as_str(),
            connection_id,
        );
    }

    let (server_read, server_write) = accepted_server_connection.tcp_stream.into_split();

    tokio::spawn(copy_from_connection_to_gateway(
        server_read,
        proxy_write,
        listening_addr,
        configuration.debug,
    ));
    tokio::spawn(copy_from_gateway_to_connection(
        server_write,
        proxy_read,
        listening_addr,
        configuration.debug,
    ));
}

async fn copy_from_connection_to_gateway(
    mut server_read: OwnedReadHalf,
    mut proxy_write: ProxyConnectionWriteHalf,
    listening_addr: SocketAddr,
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
                    proxy_write.connection_id,
                    proxy_write.gateway_id.as_str(),
                    err
                );

                if debug {
                    println!("{}", err);
                }

                proxy_write.disconnect();
                break;
            }
        };

        if read_size == 0 {
            let err = format!(
                "Reading from {} is closed. Closing connection {} on gateway {}",
                listening_addr,
                proxy_write.connection_id,
                proxy_write.gateway_id.as_str(),
            );

            if debug {
                println!("{}", err);
            }
            proxy_write.disconnect();
            break;
        }

        let result = proxy_write.write_all(&buffer[..read_size]).await;

        if let Err(err) = result {
            let err = format!(
                "Error writing to proxy connection {}-{} width id {}. Err: {:?}",
                proxy_write.gateway_id.as_str(),
                proxy_write.remote_host.as_str(),
                proxy_write.connection_id,
                err
            );

            if debug {
                println!("{}", err);
            }
            proxy_write.disconnect();
            break;
        }
    }
}

async fn copy_from_gateway_to_connection(
    mut server_write: OwnedWriteHalf,
    mut proxy_read: ProxyConnectionReadHalf,
    listening_addr: SocketAddr,
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
                    proxy_read.gateway_id.as_str(),
                    proxy_read.remote_endpoint,
                    proxy_read.connection_id,
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

        let write_future = server_write.write_all(&buffer[..payload_size]);

        let result = tokio::time::timeout(Duration::from_secs(30), write_future).await;

        if result.is_err() {
            let err = format!(
                "Write from gateway:{}->{} with connection id {} to {} is ended with timeout. Closing connection",
                proxy_read.gateway_id.as_str(),
                proxy_read.remote_endpoint.as_str(),
                proxy_read.connection_id,
                listening_addr
            );

            if debug {
                println!("{}", err);
            }

            break;
        }

        let result = result.unwrap();

        if let Err(err) = result {
            let err = format!(
                "Write from gateway:{}->{} with connection id {} to {} is ended with error: {:?}. Closing connection",
                proxy_read.gateway_id.as_str(),
                proxy_read.remote_endpoint.as_str(),
                proxy_read.connection_id,
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
