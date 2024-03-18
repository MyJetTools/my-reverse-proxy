use std::{sync::Arc, time::Duration};

use rust_extensions::date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::Mutex,
};

pub fn start(listen_addr: std::net::SocketAddr, remote_addr: std::net::SocketAddr) {
    tokio::spawn(tcp_server_accept_loop(listen_addr, remote_addr));
}

async fn tcp_server_accept_loop(
    listen_addr: std::net::SocketAddr,
    remote_addr: std::net::SocketAddr,
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

        let remote_tcp_connection_result = TcpStream::connect(remote_addr).await;

        if let Err(err) = remote_tcp_connection_result {
            println!(
                "Error connecting to remote tcp {} server: {:?}. Closing incoming connection: {}",
                remote_addr, err, socket_addr
            );
            let _ = server_stream.shutdown().await;
            continue;
        }

        tokio::spawn(connection_loop(
            listen_addr,
            remote_addr,
            server_stream,
            remote_tcp_connection_result.unwrap(),
        ));
    }
}

async fn connection_loop(
    listen_addr: std::net::SocketAddr,
    remote_addr: std::net::SocketAddr,
    server_stream: TcpStream,
    remote_stream: TcpStream,
) {
    let (tcp_server_reader, tcp_server_writer) = server_stream.into_split();

    let (remote_tcp_read, remote_tcp_writer) = remote_stream.into_split();

    let tcp_server_writer = Arc::new(Mutex::new(tcp_server_writer));

    let remote_tcp_writer = Arc::new(Mutex::new(remote_tcp_writer));

    let incoming_traffic_moment = Arc::new(AtomicDateTimeAsMicroseconds::now());

    tokio::spawn(copy_to_remote_loop(
        tcp_server_reader,
        remote_tcp_writer.clone(),
        incoming_traffic_moment.clone(),
    ));
    tokio::spawn(copy_from_remote_loop(
        remote_tcp_read,
        tcp_server_writer.clone(),
    ));

    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;

        let now = DateTimeAsMicroseconds::now();

        let last_incoming_traffic =
            DateTimeAsMicroseconds::new(incoming_traffic_moment.get_unix_microseconds());

        if now
            .duration_since(last_incoming_traffic)
            .as_positive_or_zero()
            > Duration::from_secs(60)
        {
            println!(
                "Dead Tcp PortForward {}->{} connection detected. Closing",
                listen_addr, remote_addr
            );

            {
                let mut remote_tcp_writer = remote_tcp_writer.lock().await;
                let _ = remote_tcp_writer.shutdown().await;
            }

            {
                let mut tcp_server_writer = tcp_server_writer.lock().await;
                let _ = tcp_server_writer.shutdown().await;
            }

            break;
        }
    }
}

async fn copy_to_remote_loop(
    mut tcp_server_reader: OwnedReadHalf,
    remote_tcp_writer: Arc<Mutex<OwnedWriteHalf>>,
    incoming_traffic_moment: Arc<AtomicDateTimeAsMicroseconds>,
) {
    let mut buf = Vec::with_capacity(crate::settings::BUFFER_SIZE);

    unsafe {
        buf.set_len(crate::settings::BUFFER_SIZE);
    }

    loop {
        let n = tcp_server_reader.read(&mut buf).await.unwrap();

        let mut remote_tcp_writer_access = remote_tcp_writer.lock().await;
        if n == 0 {
            let _ = remote_tcp_writer_access.shutdown().await;
            break;
        }
        incoming_traffic_moment.update(DateTimeAsMicroseconds::now());

        let result = remote_tcp_writer_access.write_all(&buf[0..n]).await;

        if result.is_err() {
            let _ = remote_tcp_writer_access.shutdown().await;
        }
    }
}

async fn copy_from_remote_loop(
    mut remote_server_reader: OwnedReadHalf,
    tcp_server_writer: Arc<Mutex<OwnedWriteHalf>>,
) {
    let mut buf = Vec::with_capacity(crate::settings::BUFFER_SIZE);

    unsafe {
        buf.set_len(crate::settings::BUFFER_SIZE);
    }

    loop {
        let n = remote_server_reader.read(&mut buf).await.unwrap();

        let mut tcp_server_writer_access = tcp_server_writer.lock().await;
        if n == 0 {
            let _ = tcp_server_writer_access.shutdown().await;
            break;
        }
        let result = tcp_server_writer_access.write_all(&buf[0..n]).await;

        if result.is_err() {
            let _ = tcp_server_writer_access.shutdown().await;
        }
    }
}
