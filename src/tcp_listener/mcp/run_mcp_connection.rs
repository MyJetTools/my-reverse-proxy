use std::{sync::Arc, time::Duration};

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

use crate::{
    configurations::McpEndpointHostConfig,
    tcp_listener::AcceptedTcpConnection,
    tcp_or_unix::{MyNetworkStream, MyOwnedReadHalf, MyOwnedWriteHalf},
};

const BUFFER_LEN: usize = 512 * 1024;

const READ_TIMEOUT: Duration = Duration::from_secs(10);

const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn run_mcp_connection(
    mut accepted_tcp_connection: AcceptedTcpConnection,
    remote_host: &Arc<RemoteEndpointOwned>,
    configuration: &Arc<McpEndpointHostConfig>,
) {
    if configuration.debug {
        println!("Accepted mcp connection forwarded to",);
    }

    let connect_result = tokio::net::TcpStream::connect(remote_host.as_str()).await;

    let tcp_stream = match connect_result {
        Ok(remote_host) => remote_host,
        Err(err) => {
            println!(
                "Can not connect to mcp remote host `{}`. Err: {:?}",
                remote_host.as_str(),
                err
            );
            accepted_tcp_connection.network_stream.shutdown().await;
            return;
        }
    };

    let remote_host = MyNetworkStream::Tcp(tcp_stream);

    let (read_remote_host, write_remote_host) = remote_host.into_split();

    let (accepted_connection_read, accepted_connection_write) =
        accepted_tcp_connection.network_stream.into_split();

    tokio::spawn(link_tcp_streams(
        accepted_connection_read,
        write_remote_host,
        "Server to Client",
    ));

    tokio::spawn(link_tcp_streams(
        read_remote_host,
        accepted_connection_write,
        "Client to Server",
    ));

    /*
    let result = read_first_payload(&mut accepted_tcp_connection, configuration.as_ref())
        .await
        .unwrap();
     */

    //  let str = std::str::from_utf8(result.as_slice()).unwrap();

    println!("")
}

async fn link_tcp_streams(
    mut read_stream: MyOwnedReadHalf,
    mut write_stream: MyOwnedWriteHalf,
    marker: &'static str,
) {
    let mut read_buffer = Vec::with_capacity(BUFFER_LEN);
    unsafe {
        read_buffer.set_len(BUFFER_LEN);
    }

    loop {
        let read_result = read_stream
            .read_with_timeout(&mut read_buffer, READ_TIMEOUT)
            .await;

        let read_size = match read_result {
            Ok(read_size) => read_size,
            Err(err) => {
                write_stream.shutdown().await;
                println!("Mcp Read/Write loop is stopped. Error: {:?}", err);
                return;
            }
        };
        let buffer_to_write = &read_buffer.as_slice()[..read_size];

        println!("---{marker}--- Start");
        println!("{:?}", std::str::from_utf8(buffer_to_write));
        println!("---{marker}--- End");

        if write_stream
            .write_all_with_timeout(buffer_to_write, WRITE_TIMEOUT)
            .await
            .is_err()
        {
            break;
        }
    }
}

/*
async fn read_first_payload(
    accepted_tcp_connection: &mut AcceptedTcpConnection,
    config: &McpEndpointHostConfig,
) -> Result<Vec<u8>, String> {
    let mut read_buffer = Vec::with_capacity(BUFFER_LEN);
    unsafe {
        read_buffer.set_len(BUFFER_LEN);
    }

    let load_size = accepted_tcp_connection
        .network_stream
        .read(&mut read_buffer)
        .await?;

    unsafe {
        read_buffer.set_len(load_size);
    }

    Ok(read_buffer)
}
 */
