use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http1::SendRequest;

use my_ssh::SshCredentials;

use crate::ssh_to_http_port_forward::SshToHttpPortForwardConfiguration;
use crate::{app::AppContext, http_proxy_pass::ProxyPassError};

use crate::configurations::*;

pub async fn connect_to_http_over_ssh_with_tunnel(
    app: &AppContext,
    ssh_credentials: &Arc<SshCredentials>,
    remote_host: &RemoteHost,
) -> Result<
    (
        SendRequest<Full<Bytes>>,
        Arc<SshToHttpPortForwardConfiguration>,
    ),
    ProxyPassError,
> {
    let tunnel_info = crate::ssh_to_http_port_forward::create_port_forward(
        ssh_credentials,
        remote_host.get_host(),
        remote_host.get_port(),
        || app.local_port_allocator.next(),
    )
    .await;

    let remote_host = tunnel_info.get_unix_socket_path();

    let result = super::connect_to_http_unix_socket_endpoint::connect_to_http_unix_socket_endpoint(
        &remote_host,
    )
    .await?;

    return Ok((result, tunnel_info));
}
