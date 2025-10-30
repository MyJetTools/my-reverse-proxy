use std::{net::SocketAddr, sync::Arc};

use my_ssh::SshCredentials;
use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{app::APP_CTX, configurations::*, tcp_listener::AcceptedTcpConnection};

pub async fn handle_connection(
    mut accepted_server_connection: AcceptedTcpConnection,
    _listening_addr: SocketAddr,
    configuration: Arc<TcpEndpointHostConfig>,
    ssh_credentials: &Arc<SshCredentials>,
    remote_endpoint: Arc<RemoteEndpointOwned>,
) {
    let ssh_session_handler = APP_CTX.ssh_sessions_pool.get(ssh_credentials).await;

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

    let ssh_channel = ssh_session_handler
        .ssh_session
        .connect_to_remote_host(
            remote_endpoint.get_host(),
            remote_port.unwrap(),
            crate::app::APP_CTX
                .connection_settings
                .remote_connect_timeout,
        )
        .await;

    let ssh_channel = match ssh_channel {
        Ok(value) => value,
        Err(err) => {
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
    };

    tokio::spawn(super::handle_port_forward(
        accepted_server_connection,
        ssh_channel,
        None,
    ));
}

/*
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
    ));
    tokio::spawn(super::forwards::copy_loop(
        remote_reader,
        tcp_server_writer.clone(),
        incoming_traffic_moment.clone(),
        buffer_size,
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
 */
