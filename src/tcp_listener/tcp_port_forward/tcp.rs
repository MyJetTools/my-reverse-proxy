use std::{net::SocketAddr, sync::Arc};

use rust_extensions::{
    date_time::AtomicDateTimeAsMicroseconds, remote_endpoint::RemoteEndpointOwned,
};
use tokio::{net::TcpStream, sync::Mutex};

use crate::{configurations::*, tcp_listener::AcceptedTcpConnection, tcp_or_unix::*};

pub async fn handle_connection(
    mut accepted_server_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
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

    if let Err(err) = remote_tcp_connection_result {
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

    tokio::spawn(handle_port_forward(
        listening_addr,
        remote_host,
        accepted_server_connection.network_stream,
        remote_tcp_connection_result.unwrap().into(),
        crate::app::APP_CTX.connection_settings.buffer_size,
        configuration.debug,
    ));
}

async fn handle_port_forward(
    listen_addr: std::net::SocketAddr,
    remote_host: Arc<RemoteEndpointOwned>,
    server_stream: MyNetworkStream,
    remote_stream: MyNetworkStream,
    buffer_size: usize,
    debug: bool,
) {
    let (server_reader, server_writer) = server_stream.into_split();

    let (remote_reader, remote_writer) = remote_stream.into_split();

    let tcp_server_writer = Arc::new(Mutex::new(server_writer));

    let remote_tcp_writer = Arc::new(Mutex::new(remote_writer));

    let incoming_traffic_moment = Arc::new(AtomicDateTimeAsMicroseconds::now());

    tokio::spawn(super::forwards::copy_loop(
        server_reader,
        remote_tcp_writer.clone(),
        incoming_traffic_moment.clone(),
        buffer_size,
        debug,
    ));
    tokio::spawn(super::forwards::copy_loop(
        remote_reader,
        tcp_server_writer.clone(),
        incoming_traffic_moment.clone(),
        buffer_size,
        debug,
    ));

    super::forwards::await_while_alive(
        tcp_server_writer,
        remote_tcp_writer,
        incoming_traffic_moment,
        || {
            if debug {
                println!(
                    "Dead Tcp PortForward {}->{} connection detected. Closing",
                    listen_addr,
                    remote_host.as_str()
                );
            }
        },
    )
    .await;
}
