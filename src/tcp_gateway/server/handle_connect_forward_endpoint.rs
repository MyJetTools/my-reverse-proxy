use std::{sync::Arc, time::Duration};

use tokio::{
    io::AsyncReadExt,
    net::{tcp::OwnedReadHalf, TcpStream},
};

use crate::tcp_gateway::*;

use super::{TcpGatewayServerConnection, TcpGatewayServerForwardConnection};

pub async fn handle_connect_forward_endpoint(
    connection_id: u32,
    remote_addr: String,
    connect_timeout: Duration,
    gateway_connection: Arc<TcpGatewayServerConnection>,
) {
    let connect_feature = TcpStream::connect(remote_addr.as_str());

    let connect_result = tokio::time::timeout(connect_timeout, connect_feature).await;

    if connect_result.is_err() {
        let err = format!("Timeout: {:?}", connect_timeout);
        let payload_to_send = TcpGatewayContract::ConnectionError {
            connection_id,
            error: err.as_str(),
        }
        .to_vec();

        gateway_connection
            .send_payload(payload_to_send.as_slice())
            .await;

        return;
    }

    let tcp_stream = match connect_result.unwrap() {
        Ok(tcp_stream) => tcp_stream,
        Err(err) => {
            let err = format!("{:?}", err);
            let payload_to_send = TcpGatewayContract::ConnectionError {
                connection_id,
                error: err.as_str(),
            }
            .to_vec();

            gateway_connection
                .send_payload(payload_to_send.as_slice())
                .await;

            return;
        }
    };

    let payload = TcpGatewayContract::Connected { connection_id }.to_vec();

    gateway_connection.send_payload(payload.as_slice()).await;

    let (read, write) = tcp_stream.into_split();

    let tcp_server_gateway_forward_connection = TcpGatewayServerForwardConnection::new(
        gateway_connection.gateway_id.clone(),
        Arc::new(remote_addr),
        write,
    );

    let tcp_server_gateway_forward_connection = Arc::new(tcp_server_gateway_forward_connection);

    gateway_connection
        .add_forward_connection(connection_id, tcp_server_gateway_forward_connection.clone())
        .await;

    tokio::spawn(read_loop(
        tcp_server_gateway_forward_connection.clone(),
        read,
        connection_id,
    ));
}

async fn read_loop(
    gateway_connection: Arc<TcpGatewayServerForwardConnection>,
    mut read_stream: OwnedReadHalf,
    connection_id: u32,
) {
    let mut buffer: Vec<u8> = Vec::with_capacity(1024 * 1024);

    unsafe {
        buffer.set_len(1024 * 1024);
    }

    let mut err_to_send = String::new();
    loop {
        let read_result = read_stream.read(buffer.as_mut()).await;

        let size = match read_result {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                size
            }
            Err(err) => {
                println!(
                    "Failed to read from Remote connection '{}' for TCP Gateway. Err: {:?}",
                    gateway_connection.remote_addr.as_str(),
                    err
                );

                err_to_send = format!("{:?}", err);
                break;
            }
        };

        let send_payload = TcpGatewayContract::SendPayload {
            connection_id,
            payload: &buffer[0..size],
        }
        .to_vec();

        gateway_connection
            .send_payload(send_payload.as_slice())
            .await;
    }

    let disconnected_payload = TcpGatewayContract::ConnectionError {
        connection_id,
        error: err_to_send.as_str(),
    }
    .to_vec();

    gateway_connection
        .send_payload(disconnected_payload.as_slice())
        .await;
}
