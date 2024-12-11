use std::{net::SocketAddr, sync::Arc};

use my_ssh::{SshAsyncChannel, SshCredentials};
use rust_extensions::date_time::AtomicDateTimeAsMicroseconds;
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::Mutex};

use crate::{app::AppContext, configurations::*, tcp_listener::AcceptedTcpConnection};

/*
pub fn start_tcp_over_ssh(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    endpoint_info: Arc<TcpOverSshEndpointHostConfig>,
) {
    tokio::spawn(tcp_server_accept_loop(app, listen_addr, endpoint_info));
}

async fn tcp_server_accept_loop(
    app: Arc<AppContext>,
    listen_addr: std::net::SocketAddr,
    endpoint_info: Arc<TcpOverSshEndpointHostConfig>,
) {
    let listener = tokio::net::TcpListener::bind(listen_addr).await;

    if let Err(err) = listener {
        println!(
            "Error binding to tcp port {} for forwarding to {}->{} has Error: {:?}",
            listen_addr,
            endpoint_info.ssh_credentials.to_string(),
            endpoint_info.remote_host.as_str(),
            err
        );
        return;
    }

    let listener = listener.unwrap();

    println!(
        "Enabled PortForward: {}->{}->{}",
        listen_addr,
        endpoint_info.ssh_credentials.to_string(),
        endpoint_info.remote_host.as_str()
    );

    loop {
        let (mut server_stream, socket_addr) = listener.accept().await.unwrap();

        if app.states.is_shutting_down() {
            return;
        }

        let ssh_session = SshSession::new(endpoint_info.ssh_credentials.clone());

        let ssh_channel = ssh_session
            .connect_to_remote_host(
                endpoint_info.remote_host.get_host(),
                endpoint_info.remote_host.get_port(),
                app.connection_settings.remote_connect_timeout,
            )
            .await;

        if let Err(err) = ssh_channel {
            if endpoint_info.debug {
                println!(
                    "Error connecting to remote tcp {} over ssh {}->{} server. Closing incoming connection: {}. Err: {:?}",
                    listen_addr.to_string(),
                    endpoint_info.ssh_credentials.to_string(),
                    endpoint_info.remote_host.as_str(),
                    socket_addr,
                    err
                );
            }
            let _ = server_stream.shutdown().await;
            continue;
        }

        tokio::spawn(connection_loop(
            listen_addr,
            endpoint_info.ssh_credentials.clone(),
            endpoint_info.remote_host.clone(),
            server_stream,
            ssh_channel.unwrap(),
            app.connection_settings.buffer_size,
            endpoint_info.debug,
        ));
    }
}
 */
pub async fn handle_connection(
    app: Arc<AppContext>,
    mut accepted_server_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<TcpEndpointHostConfig>,
) {
    let ssh_credentials = configuration.remote_host.ssh_credentials.clone().unwrap();
    let ssh_session = my_ssh::SSH_SESSIONS_POOL
        .get_or_create(&ssh_credentials)
        .await;

    let remote_port = configuration.remote_host.get_remote_endpoint().get_port();

    if remote_port.is_none() {
        println!(
            "Remote port is not set for tcp port forward {}. Closing incoming connection: {}",
            configuration.host_endpoint.as_str(),
            accepted_server_connection.addr
        );
        let _ = accepted_server_connection.tcp_stream.shutdown().await;
        return;
    }

    let ssh_channel = ssh_session
        .connect_to_remote_host(
            configuration.remote_host.get_remote_endpoint().get_host(),
            remote_port.unwrap(),
            app.connection_settings.remote_connect_timeout,
        )
        .await;

    if let Err(err) = ssh_channel {
        if configuration.debug {
            println!(
                    "Error connecting to remote tcp endpoint over ssh {}->{} server. Closing incoming connection: {}. Err: {:?}",
                    ssh_credentials.to_string(),
                    configuration.remote_host.get_remote_endpoint().as_str(),
                    accepted_server_connection.addr,
                    err
                );
        }
        let _ = accepted_server_connection.tcp_stream.shutdown().await;
        return;
    }

    let remote_host = Arc::new(
        configuration
            .remote_host
            .get_remote_endpoint()
            .as_str()
            .to_string(),
    );

    tokio::spawn(connection_loop(
        listening_addr,
        ssh_credentials.clone(),
        remote_host,
        accepted_server_connection.tcp_stream,
        ssh_channel.unwrap(),
        app.connection_settings.buffer_size,
        configuration.debug,
    ));
}

async fn connection_loop(
    listen_addr: SocketAddr,
    ssh_credentials: Arc<SshCredentials>,
    remote_host: Arc<String>,
    server_stream: TcpStream,
    remote_stream: SshAsyncChannel,
    buffer_size: usize,
    debug: bool,
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
            if debug {
                println!(
                    "Dead Tcp PortForward {}->{}->{} connection detected. Closing",
                    listen_addr,
                    ssh_credentials.to_string(),
                    remote_host.as_str()
                );
            }
        },
    )
    .await;
}