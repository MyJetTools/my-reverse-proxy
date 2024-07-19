use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http2::SendRequest;
use my_ssh::SshCredentials;

use crate::{app::AppContext, http_proxy_pass::ProxyPassError};

use crate::configurations::*;

pub async fn connect_to_http2_over_ssh(
    app: &AppContext,
    ssh_credentials: &Arc<SshCredentials>,
    ssh_remote_host: &RemoteHost,
) -> Result<SendRequest<Full<Bytes>>, ProxyPassError> {
    let tunnel_info = app
        .ssh_to_http_port_forward_pool
        .get_or_create_port_forward(
            ssh_credentials,
            ssh_remote_host.get_host(),
            ssh_remote_host.get_port(),
            || app.local_port_allocator.next(),
        )
        .await;

    let remote_host = tunnel_info.get_unix_socket_path();

    let result =
        super::connect_to_http2_unix_socket_endpoint::connect_to_http2_unix_socket_endpoint(
            &remote_host,
        )
        .await?;

    return Ok(result);

    /*
    let ssh_session = Arc::new(SshSession::new(ssh_credentials.clone()));

    let ssh_channel = ssh_session
        .connect_to_remote_host(
            ssh_remote_host.get_host(),
            ssh_remote_host.get_port(),
            app.connection_settings.remote_connect_timeout,
        )
        .await?;

    let buf_writer = tokio::io::BufWriter::with_capacity(
        app.connection_settings.buffer_size,
        tokio::io::BufReader::with_capacity(app.connection_settings.buffer_size, ssh_channel),
    );

    let io = TokioIo::new(buf_writer);

    let (mut sender, conn) =
        hyper::client::conn::http2::handshake(TokioExecutor::new(), io).await?;

    let proxy_pass_uri = ssh_remote_host.to_string();

    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!(
                "Http Connection to http://{} is failed: {:?}",
                proxy_pass_uri, err
            );
        }

        //Here
    });

    sender.ready().await?;

    Ok((ssh_session, sender))
     */
}
