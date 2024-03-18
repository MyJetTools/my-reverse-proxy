use std::sync::Arc;

use rust_extensions::date_time::AtomicDateTimeAsMicroseconds;
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::Mutex};

use crate::{app::AppContext, settings::SshConfiguration};

pub fn start_tcp_over_ssh(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    ssh_configuration: SshConfiguration,
) {
    tokio::spawn(tcp_server_accept_loop(app, listen_addr, ssh_configuration));
}

async fn tcp_server_accept_loop(
    app: Arc<AppContext>,
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
                app.connection_settings.remote_connect_timeout,
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
            app.connection_settings.buffer_size,
        ));
    }
}

async fn connection_loop(
    listen_addr: std::net::SocketAddr,
    ssh_configuration: Arc<SshConfiguration>,
    server_stream: TcpStream,
    remote_stream: my_ssh::ssh2::Channel,
    buffer_size: usize,
) {
    let (tcp_server_reader, tcp_server_writer) = server_stream.into_split();

    let (remote_ssh_read, remote_ssh_writer) = my_ssh::async_ssh_channel::split(remote_stream);

    let tcp_server_writer = Arc::new(Mutex::new(tcp_server_writer));

    let remote_ssh_writer = Arc::new(Mutex::new(remote_ssh_writer));

    let incoming_traffic_moment = Arc::new(AtomicDateTimeAsMicroseconds::now());

    tokio::spawn(super::forwards::copy_loop(
        tcp_server_reader,
        remote_ssh_writer.clone(),
        incoming_traffic_moment.clone(),
        buffer_size,
    ));
    tokio::spawn(super::forwards::copy_loop(
        remote_ssh_read,
        tcp_server_writer.clone(),
        incoming_traffic_moment.clone(),
        buffer_size,
    ));

    super::forwards::await_while_alive(
        tcp_server_writer,
        remote_ssh_writer,
        incoming_traffic_moment,
        || {
            println!(
                "Dead Tcp PortForward {}->{} connection detected. Closing",
                listen_addr,
                ssh_configuration.to_string()
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
