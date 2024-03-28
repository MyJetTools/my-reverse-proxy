use std::sync::Arc;

use rust_extensions::date_time::AtomicDateTimeAsMicroseconds;
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::Mutex};

use crate::{app::AppContext, types::WhiteListedIpList};

pub fn start_tcp(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    remote_addr: std::net::SocketAddr,
    whitelisted_ip: WhiteListedIpList,
    debug: bool,
) {
    tokio::spawn(tcp_server_accept_loop(
        app,
        listen_addr,
        remote_addr,
        whitelisted_ip,
        debug,
    ));
}

async fn tcp_server_accept_loop(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    remote_addr: std::net::SocketAddr,
    whitelisted_ip: WhiteListedIpList,
    debug: bool,
) {
    let listener = tokio::net::TcpListener::bind(listen_addr).await;

    if let Err(err) = listener {
        println!(
            "Error binding to tcp port {} for forwarding to {} has Error: {:?}",
            listen_addr, remote_addr, err
        );
        return;
    }

    let listener = listener.unwrap();

    println!("Enabled PortForward: {} -> {}", listen_addr, remote_addr);

    loop {
        let (mut server_stream, socket_addr) = listener.accept().await.unwrap();

        if !whitelisted_ip.is_whitelisted(&socket_addr.ip()) {
            if debug {
                println!(
                    "Incoming connection from {} is not whitelisted. Closing it",
                    socket_addr
                );
            }

            let _ = server_stream.shutdown().await;
            continue;
        }

        let remote_tcp_connection_result = tokio::time::timeout(
            app.connection_settings.remote_connect_timeout,
            TcpStream::connect(remote_addr),
        )
        .await;

        if remote_tcp_connection_result.is_err() {
            if debug {
                println!(
                    "Timeout while connecting to remote tcp {} server. Closing incoming connection: {}",
                    remote_addr, socket_addr
                );
            }
            let _ = server_stream.shutdown().await;
            continue;
        }

        let remote_tcp_connection_result = remote_tcp_connection_result.unwrap();

        if let Err(err) = remote_tcp_connection_result {
            if debug {
                println!(
                    "Error connecting to remote tcp {} server: {:?}. Closing incoming connection: {}",
                    remote_addr, err, socket_addr
                );
            }
            let _ = server_stream.shutdown().await;
            continue;
        }

        tokio::spawn(connection_loop(
            listen_addr,
            remote_addr,
            server_stream,
            remote_tcp_connection_result.unwrap(),
            app.connection_settings.buffer_size,
            debug,
        ));
    }
}

async fn connection_loop(
    listen_addr: std::net::SocketAddr,
    remote_addr: std::net::SocketAddr,
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
                    listen_addr, remote_addr
                );
            }
        },
    )
    .await;
}
