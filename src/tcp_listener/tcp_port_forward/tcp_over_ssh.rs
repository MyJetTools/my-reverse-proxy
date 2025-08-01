use std::{net::SocketAddr, sync::Arc};

use my_ssh::SshCredentials;
use rust_extensions::{
    date_time::AtomicDateTimeAsMicroseconds, remote_endpoint::RemoteEndpointOwned,
};
use tokio::sync::Mutex;

use crate::{configurations::*, tcp_listener::AcceptedTcpConnection, tcp_or_unix::MyNetworkStream};

pub async fn handle_connection(
    mut accepted_server_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<TcpEndpointHostConfig>,
    ssh_credentials: Arc<SshCredentials>,
    remote_endpoint: Arc<RemoteEndpointOwned>,
) {
    let ssh_session = my_ssh::SSH_SESSIONS_POOL
        .get_or_create(&ssh_credentials)
        .await;

    let remote_port = remote_endpoint.get_port();

    if remote_port.is_none() {
        println!(
            "Remote port is not set for tcp port forward {}. Closing incoming connection: {}",
            configuration.host_endpoint.as_str(),
            accepted_server_connection.addr
        );
        let _ = accepted_server_connection.network_stream.shutdown().await;
        return;
    }

    let ssh_channel = ssh_session
        .connect_to_remote_host(
            remote_endpoint.get_host(),
            remote_port.unwrap(),
            crate::app::APP_CTX
                .connection_settings
                .remote_connect_timeout,
        )
        .await;

    if let Err(err) = ssh_channel {
        if configuration.debug {
            println!(
                    "Error connecting to remote tcp endpoint over ssh {}->{} server. Closing incoming connection: {}. Err: {:?}",
                    ssh_credentials.to_string(),
                    remote_endpoint.as_str(),
                    accepted_server_connection.addr,
                    err
                );
        }
        let _ = accepted_server_connection.network_stream.shutdown().await;
        return;
    }

    let remote_host = Arc::new(remote_endpoint.as_str().to_string());

    tokio::spawn(connection_loop(
        listening_addr,
        ssh_credentials.clone(),
        remote_host,
        accepted_server_connection.network_stream,
        ssh_channel.unwrap().into(),
        crate::app::APP_CTX.connection_settings.buffer_size,
        configuration.debug,
    ));
}

async fn connection_loop(
    listen_addr: SocketAddr,
    ssh_credentials: Arc<SshCredentials>,
    remote_host: Arc<String>,
    server_stream: MyNetworkStream,
    remote_stream: MyNetworkStream,
    buffer_size: usize,
    debug: bool,
) {
    let (tcp_server_reader, tcp_server_writer) = server_stream.into_split();

    let (remote_reader, remote_writer) = remote_stream.into_split();

    let tcp_server_writer = Arc::new(Mutex::new(tcp_server_writer));

    let remote_ssh_writer = Arc::new(Mutex::new(remote_writer));

    let incoming_traffic_moment = Arc::new(AtomicDateTimeAsMicroseconds::now());

    tokio::spawn(super::forwards::copy_loop(
        tcp_server_reader,
        remote_ssh_writer.clone(),
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
