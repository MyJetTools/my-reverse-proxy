use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http1::{Builder, SendRequest};
use hyper_util::rt::TokioIo;
use my_ssh::{SshCredentials, SshSession};

use crate::{app::AppContext, http_proxy_pass::ProxyPassError};

use crate::configurations::*;

pub async fn connect_to_http_over_ssh_with_tunnel(
    app: &AppContext,
    ssh_credentials: &Arc<SshCredentials>,
    remote_host: &RemoteHost,
) -> Result<SendRequest<Full<Bytes>>, ProxyPassError> {
    let tunnel_info = app
        .ssh_to_http_port_forward_pool
        .get_or_create_port_forward(
            ssh_credentials,
            remote_host.get_host(),
            remote_host.get_port(),
            || app.local_port_allocator.next(),
        )
        .await;

    //let remote_host = tunnel_info.get_unix_socket_path();
    let remote_host = tunnel_info.get_listen_host_port();

    /*
    let result = super::connect_to_http_unix_socket_endpoint::connect_to_http_unix_socket_endpoint(
        &remote_host,
    )
    .await?; */

    let result = super::connect_to_http_unix_socket_endpoint::connect_to_http_localhost_endpoint(
        &remote_host,
    )
    .await?;

    return Ok(result);
}

pub async fn connect_to_http_over_ssh(
    ssh_credentials: &Arc<SshCredentials>,
    remote_host: &RemoteHost,
) -> Result<(SendRequest<Full<Bytes>>, SshSession), ProxyPassError> {
    let ssh_session = my_ssh::SshSession::new(ssh_credentials.clone());

    println!("Ssh Session is established");

    let connect_result = ssh_session
        .connect_to_remote_host(
            remote_host.get_host(),
            remote_host.get_port(),
            Duration::from_secs(10),
        )
        .await;

    match connect_result {
        Ok(tcp_stream) => {
            let io = TokioIo::new(tcp_stream);
            let handshake_result = Builder::new()
                .max_buf_size(1024 * 1024 * 5)
                .handshake(io)
                .await;
            match handshake_result {
                Ok((mut sender, conn)) => {
                    let remote_host = remote_host.to_string();
                    tokio::task::spawn(async move {
                        if let Err(err) = conn.with_upgrades().await {
                            println!("Http Connection to {} is failed: {:?}", remote_host, err);
                        }

                        //Here
                    });

                    sender.ready().await?;
                    return Ok((sender, ssh_session));
                }
                Err(err) => {
                    return Err(ProxyPassError::HyperError(err));
                }
            }
        }
        Err(err) => {
            return Err(ProxyPassError::SshSessionError(err));
        }
    }
}
