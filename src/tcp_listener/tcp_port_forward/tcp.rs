use std::{net::SocketAddr, sync::Arc};

use rust_extensions::date_time::AtomicDateTimeAsMicroseconds;
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::Mutex};

use crate::{app::AppContext, configurations::*, tcp_listener::AcceptedTcpConnection};

/*
pub fn start_tcp(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    endpoint_info: Arc<TcpEndpointHostConfig>,
) {
    tokio::spawn(tcp_server_accept_loop(app, listen_addr, endpoint_info));
}

async fn tcp_server_accept_loop(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    endpoint_info: Arc<TcpEndpointHostConfig>,
) {
    let listener = tokio::net::TcpListener::bind(listen_addr).await;

    if let Err(err) = listener {
        println!(
            "Error binding to tcp port {} for forwarding to {} has Error: {:?}",
            listen_addr, endpoint_info.remote_addr, err
        );
        return;
    }

    let listener = listener.unwrap();

    println!(
        "Enabled PortForward: {} -> {}",
        listen_addr, endpoint_info.remote_addr
    );

    loop {
        let (mut server_stream, socket_addr) = listener.accept().await.unwrap();
    }
}
 */

pub async fn handle_connection(
    app: Arc<AppContext>,
    mut accepted_server_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<TcpEndpointHostConfig>,
) {
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

    let remote_host = Arc::new(
        configuration
            .remote_host
            .get_remote_endpoint()
            .as_str()
            .to_string(),
    );

    let remote_tcp_connection_result = tokio::time::timeout(
        app.connection_settings.remote_connect_timeout,
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
        let _ = accepted_server_connection.tcp_stream.shutdown().await;
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
        let _ = accepted_server_connection.tcp_stream.shutdown().await;
        return;
    }

    tokio::spawn(connection_loop(
        listening_addr,
        remote_host,
        accepted_server_connection.tcp_stream,
        remote_tcp_connection_result.unwrap(),
        app.connection_settings.buffer_size,
        configuration.debug,
    ));
}

async fn connection_loop(
    listen_addr: std::net::SocketAddr,
    remote_host: Arc<String>,
    server_stream: TcpStream,
    remote_stream: TcpStream,
    buffer_size: usize,
    debug: bool,
) {
    let (tcp_server_reader, tcp_server_writer) = server_stream.into_split();

    let (remote_tcp_read, remote_tcp_writer) = remote_stream.into_split();

    let tcp_server_writer = Arc::new(Mutex::new(tcp_server_writer));

    let remote_tcp_writer = Arc::new(Mutex::new(remote_tcp_writer));

    let incoming_traffic_moment = Arc::new(AtomicDateTimeAsMicroseconds::now());

    tokio::spawn(super::forwards::copy_loop(
        tcp_server_reader,
        remote_tcp_writer.clone(),
        incoming_traffic_moment.clone(),
        buffer_size,
        debug,
    ));
    tokio::spawn(super::forwards::copy_loop(
        remote_tcp_read,
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
                    listen_addr, remote_host
                );
            }
        },
    )
    .await;
}
