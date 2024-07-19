use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http1::SendRequest;
use my_ssh::SshCredentials;

use crate::{app::AppContext, http_proxy_pass::ProxyPassError};

use crate::configurations::*;

pub async fn connect_to_http_over_ssh(
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

    let remote_host: RemoteHost = format!("http://127.0.0.1:{}", tunnel_info.listen_port).into();

    let result = super::connect_to_http_endpoint::connect_to_http_endpoint(&remote_host).await?;

    return Ok(result);

    /*
    let ssh_channel = ssh_session
        .connect_to_remote_host(
            remote_host.get_host(),
            remote_host.get_port(),
            app.connection_settings.remote_connect_timeout,
        )
        .await?;

    let buf_writer = tokio::io::BufWriter::with_capacity(
        app.connection_settings.buffer_size,
        tokio::io::BufReader::with_capacity(app.connection_settings.buffer_size, ssh_channel),
    );

    let io = TokioIo::new(buf_writer);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

    let proxy_pass_uri = remote_host.to_string();

    tokio::task::spawn(async move {
        if let Err(err) = conn.with_upgrades().await {
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
