use std::sync::Arc;

use my_ssh::{SshAsyncChannel, SshCredentials, SshSession};
use rust_extensions::date_time::AtomicDateTimeAsMicroseconds;
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::Mutex};

use crate::{app::AppContext, settings::RemoteHost};

pub fn start_tcp_over_ssh(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    credentials: Arc<SshCredentials>,
    remote_host: RemoteHost,
) {
    tokio::spawn(tcp_server_accept_loop(
        app,
        listen_addr,
        credentials,
        remote_host,
    ));
}

async fn tcp_server_accept_loop(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    ssh_credentials: Arc<SshCredentials>,
    remote_host: RemoteHost,
) {
    let remote_host = Arc::new(remote_host);
    let listener = tokio::net::TcpListener::bind(listen_addr).await;

    if let Err(err) = listener {
        println!(
            "Error binding to tcp port {} for forwarding to {}->{} has Error: {:?}",
            listen_addr,
            ssh_credentials.to_string(),
            remote_host.as_str(),
            err
        );
        return;
    }

    let listener = listener.unwrap();

    println!(
        "Enabled PortForward: {}->{}->{}",
        listen_addr,
        ssh_credentials.to_string(),
        remote_host.as_str()
    );

    loop {
        let (mut server_stream, socket_addr) = listener.accept().await.unwrap();

        let ssh_session = SshSession::new(ssh_credentials.clone());

        let ssh_channel = ssh_session
            .connect_to_remote_host(
                remote_host.get_host(),
                remote_host.get_port(),
                app.connection_settings.remote_connect_timeout,
            )
            .await;

        if let Err(err) = ssh_channel {
            println!(
                "Error connecting to remote tcp {} over ssh {}->{} server. Closing incoming connection: {}. Err: {:?}",
                listen_addr.to_string(),
                ssh_credentials.to_string(),
                remote_host.as_str(),
                socket_addr,
                err
            );
            let _ = server_stream.shutdown().await;
            continue;
        }

        tokio::spawn(connection_loop(
            listen_addr,
            ssh_credentials.clone(),
            remote_host.clone(),
            server_stream,
            ssh_channel.unwrap(),
            app.connection_settings.buffer_size,
        ));
    }
}

async fn connection_loop(
    listen_addr: std::net::SocketAddr,
    ssh_credentials: Arc<SshCredentials>,
    remote_host: Arc<RemoteHost>,
    server_stream: TcpStream,
    remote_stream: SshAsyncChannel,
    buffer_size: usize,
) {
    let (tcp_server_reader, tcp_server_writer) = server_stream.into_split();

    let (remote_ssh_read, remote_ssh_writer) = futures::AsyncReadExt::split(remote_stream);

    let tcp_server_writer = Arc::new(Mutex::new(tcp_server_writer));

    let remote_ssh_writer = Arc::new(Mutex::new(remote_ssh_writer));

    let incoming_traffic_moment = Arc::new(AtomicDateTimeAsMicroseconds::now());

    tokio::spawn(super::forwards::copy_to_ssh_loop(
        tcp_server_reader,
        remote_ssh_writer.clone(),
        incoming_traffic_moment.clone(),
        buffer_size,
    ));
    tokio::spawn(super::forwards::copy_from_ssh_loop(
        remote_ssh_read,
        tcp_server_writer.clone(),
        incoming_traffic_moment.clone(),
        buffer_size,
    ));

    super::forwards::await_while_alive_with_ssh(
        tcp_server_writer,
        remote_ssh_writer,
        incoming_traffic_moment,
        || {
            println!(
                "Dead Tcp PortForward {}->{}->{} connection detected. Closing",
                listen_addr,
                ssh_credentials.to_string(),
                remote_host.as_str()
            );
        },
    )
    .await;

    /*
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
     */
}
