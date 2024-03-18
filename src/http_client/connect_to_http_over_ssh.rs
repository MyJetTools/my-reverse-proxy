use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http1::SendRequest;
use hyper_util::rt::TokioIo;
use my_ssh::{async_ssh_channel::SshAsyncChannel, SshCredentials, SshSession};

use crate::{app::AppContext, http_server::ProxyPassError};

pub async fn connect_to_http_over_ssh(
    app: &AppContext,
    ssh_credentials: Arc<SshCredentials>,
    remote_host: &str,
    remote_port: u16,
) -> Result<(Arc<SshSession>, SendRequest<Full<Bytes>>), ProxyPassError> {
    let ssh_session = Arc::new(SshSession::new(ssh_credentials.clone()));

    /*
       let ssh_session = my_ssh::SSH_SESSION_POOL
           .get_or_create_ssh_session(&ssh_credentials)
           .await;
    */
    let ssh_channel = ssh_session
        .connect_to_remote_host(
            remote_host,
            remote_port,
            app.connection_settings.remote_connect_timeout,
        )
        .await?;

    let ssh_channel = SshAsyncChannel::new(ssh_channel, app.connection_settings.buffer_size);

    let io = TokioIo::new(ssh_channel);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

    let proxy_pass_uri = format!("{}:{}", remote_host, remote_port);

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
}
