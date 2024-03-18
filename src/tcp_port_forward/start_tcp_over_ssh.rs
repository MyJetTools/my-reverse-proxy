use std::{sync::Arc, time::Duration};

use my_ssh::async_ssh_channel::{SshChannelReadHalf, SshChannelWriteHalf};
use rust_extensions::date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::Mutex,
};

use crate::settings::SshConfiguration;

pub fn start_tcp_over_ssh(listen_addr: std::net::SocketAddr, ssh_configuration: SshConfiguration) {
    tokio::spawn(tcp_server_accept_loop(listen_addr, ssh_configuration));
}

async fn tcp_server_accept_loop(
    listen_addr: std::net::SocketAddr,
    ssh_configuration: SshConfiguration,
) {
    let listener = tokio::net::TcpListener::bind(listen_addr).await;

    if let Err(err) = listener {
        println!(
            "Error binding to tcp port {} for forwarding to {} has Error: {:?}",
            listen_addr,
            ssh_configuration.to_string(),
            err
        );
        return;
    }

    let listener = listener.unwrap();

    println!(
        "Enabled PortForward: {} -> {}",
        listen_addr,
        ssh_configuration.to_string()
    );

    let ssh_configuration = Arc::new(ssh_configuration);

    let ssh_credentials = Arc::new(ssh_configuration.to_ssh_credentials());

    loop {
        let (mut server_stream, socket_addr) = listener.accept().await.unwrap();

        let ssh_session = my_ssh::SSH_SESSION_POOL
            .get_or_create_ssh_session(&ssh_credentials)
            .await;

        let ssh_channel = ssh_session
            .connect_to_remote_host(
                &ssh_configuration.remote_host,
                ssh_configuration.remote_port,
            )
            .await;

        if let Err(err) = ssh_channel {
            println!(
                "Error connecting to remote tcp over ssh '{}' server. Closing incoming connection: {}. Err: {:?}",
                ssh_configuration.to_string(),
                socket_addr,
                err
            );
            let _ = server_stream.shutdown().await;
            continue;
        }

        tokio::spawn(connection_loop(
            listen_addr,
            ssh_configuration.clone(),
            server_stream,
            ssh_channel.unwrap(),
        ));
    }
}

async fn connection_loop(
    listen_addr: std::net::SocketAddr,
    ssh_configuration: Arc<SshConfiguration>,
    server_stream: TcpStream,
    remote_stream: my_ssh::ssh2::Channel,
) {
    let (tcp_server_reader, tcp_server_writer) = server_stream.into_split();

    let (remote_ssh_read, remote_ssh_writer) = my_ssh::async_ssh_channel::split(remote_stream);

    let tcp_server_writer = Arc::new(Mutex::new(tcp_server_writer));

    let remote_ssh_writer = Arc::new(Mutex::new(remote_ssh_writer));

    let incoming_traffic_moment = Arc::new(AtomicDateTimeAsMicroseconds::now());

    tokio::spawn(copy_to_remote_loop(
        tcp_server_reader,
        remote_ssh_writer.clone(),
        incoming_traffic_moment.clone(),
    ));
    tokio::spawn(copy_from_remote_loop(
        remote_ssh_read,
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
                listen_addr,
                ssh_configuration.to_string()
            );

            {
                let remote_ssh_writer = remote_ssh_writer.lock().await;
                remote_ssh_writer.shutdown();
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
    remote_tcp_writer: Arc<Mutex<SshChannelWriteHalf>>,
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
            remote_tcp_writer_access.shutdown();
            break;
        }
        incoming_traffic_moment.update(DateTimeAsMicroseconds::now());

        let result = remote_tcp_writer_access.write_all(&buf[0..n]).await;

        if result.is_err() {
            remote_tcp_writer_access.shutdown();
        }
    }
}

async fn copy_from_remote_loop(
    mut remote_server_reader: SshChannelReadHalf,
    tcp_server_writer: Arc<Mutex<OwnedWriteHalf>>,
) {
    let mut buf = Vec::with_capacity(crate::settings::BUFFER_SIZE);

    unsafe {
        buf.set_len(crate::settings::BUFFER_SIZE);
    }

    loop {
        let read_result = remote_server_reader.read(&mut buf).await;
        let mut tcp_server_writer_access = tcp_server_writer.lock().await;

        if read_result.is_err() {
            let _ = tcp_server_writer_access.shutdown().await;
            break;
        }

        let n = read_result.unwrap();

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
