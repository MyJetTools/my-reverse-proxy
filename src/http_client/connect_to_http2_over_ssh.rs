use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http2::SendRequest;
use my_ssh::SshCredentials;

use crate::ssh_to_http_port_forward::SshToHttpPortForwardConfiguration;
use crate::{app::AppContext, http_proxy_pass::ProxyPassError};

use crate::configurations::*;

pub async fn connect_to_http2_over_ssh(
    app: &AppContext,
    ssh_credentials: &Arc<SshCredentials>,
    ssh_remote_host: &RemoteHost,
) -> Result<
    (
        SendRequest<Full<Bytes>>,
        Arc<SshToHttpPortForwardConfiguration>,
    ),
    ProxyPassError,
> {
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

    return Ok((result, tunnel_info));
}
