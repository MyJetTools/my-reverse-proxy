use tokio::io::AsyncWriteExt;

use crate::{
    configurations::{HttpEndpointInfo, McpEndpointSettings},
    network_stream::*,
};

pub async fn run_mcp_connection(
    mut tls_stream: my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    http_endpoint_info: &HttpEndpointInfo,
    connection_id: u64,
) {
    if http_endpoint_info.debug {
        println!("Accepted mcp connection",);
    }

    let remote_host = http_endpoint_info
        .locations
        .get(0)
        .unwrap()
        .proxy_pass_to
        .to_string();

    let remote_host = if remote_host.starts_with("http://") {
        &remote_host[7..]
    } else if remote_host.starts_with("https://") {
        println!("Https does not support as remote host for mcp");
        let _ = tls_stream.shutdown().await;
        return;
    } else {
        &remote_host
    };

    println!("Connecting mcp to remote host: {}", remote_host);

    let connect_result = tokio::net::TcpStream::connect(remote_host).await;

    let tcp_stream = match connect_result {
        Ok(remote_host) => remote_host,
        Err(err) => {
            println!(
                "Can not connect to mcp remote host `{}`. Err: {:?}",
                remote_host, err
            );
            let _ = tls_stream.shutdown().await;
            return;
        }
    };

    let (read_remote_host, write_remote_host) = tokio::io::split(tcp_stream);

    let (accepted_connection_read, accepted_connection_write) = tokio::io::split(tls_stream);

    let mcp_settings = http_endpoint_info.mcp_settings;

    crate::app::spawn_named(
        "mcp_link_to_server",
        link_tcp_streams(
            accepted_connection_read,
            write_remote_host,
            "->To MCP Server->",
            connection_id,
            mcp_settings,
        ),
    );

    crate::app::spawn_named(
        "mcp_link_from_server",
        link_tcp_streams(
            read_remote_host,
            accepted_connection_write,
            "<-From MCP Server<-",
            connection_id,
            mcp_settings,
        ),
    );

    /*
    let result = read_first_payload(&mut accepted_tcp_connection, configuration.as_ref())
        .await
        .unwrap();
     */

    //  let str = std::str::from_utf8(result.as_slice()).unwrap();

    println!("")
}

async fn link_tcp_streams(
    mut read_stream: impl NetworkStreamReadPart + Send + Sync + 'static,
    mut write_stream: impl NetworkStreamWritePart + Send + Sync + 'static,
    marker: &'static str,
    connection_id: u64,
    mcp_settings: McpEndpointSettings,
) {
    let mut read_buffer = Vec::with_capacity(mcp_settings.buffer_size);
    unsafe {
        read_buffer.set_len(mcp_settings.buffer_size);
    }

    loop {
        let read_result = read_stream
            .read_with_timeout(&mut read_buffer, mcp_settings.read_timeout)
            .await;

        let read_size = match read_result {
            Ok(read_size) => read_size,
            Err(err) => {
                write_stream.shutdown_socket().await;
                println!(
                    "{connection_id} Mcp pump {marker} stopped. Error: {:?}",
                    err
                );
                return;
            }
        };

        if read_size == 0 {
            println!("{connection_id} Mcp pump {marker} stopped gracefully");
            return;
        }
        let buffer_to_write = &read_buffer.as_slice()[..read_size];

        if write_stream
            .write_all_with_timeout(buffer_to_write, mcp_settings.write_timeout)
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
