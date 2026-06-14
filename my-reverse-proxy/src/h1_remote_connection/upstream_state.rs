use crate::configurations::{MyReverseProxyRemoteEndpoint, ProxyPassToConfig, ProxyPassToModel};

/// The upstream path an MCP location rewrites requests onto, if this is one.
/// Depends only on the config, not on the chosen connection.
pub fn mcp_path(proxy_pass_to: &ProxyPassToConfig) -> Option<&str> {
    match proxy_pass_to {
        ProxyPassToConfig::McpHttp1(model) => Some(model.remote_host.get_path_and_query()),
        _ => None,
    }
}

/// Identity of an upstream connection: remote host + protocol. Used as the pool
/// key — two locations with the same key share the upstream connection slot.
pub fn connection_key(proxy_pass_to: &ProxyPassToConfig) -> String {
    match proxy_pass_to {
        ProxyPassToConfig::Http1(model) | ProxyPassToConfig::McpHttp1(model) => {
            remote_host_key("h1", model)
        }
        ProxyPassToConfig::Http2(model) => remote_host_key("h2", model),
        ProxyPassToConfig::UnixHttp1(model) => remote_host_key("uds-h1", model),
        ProxyPassToConfig::UnixHttp2(model) => remote_host_key("uds-h2", model),
        other => other.to_string(),
    }
}

fn remote_host_key(protocol: &str, model: &ProxyPassToModel) -> String {
    match &model.remote_host {
        MyReverseProxyRemoteEndpoint::Direct { remote_host } => format!(
            "{protocol}|{:?}|{}",
            remote_host.get_scheme(),
            remote_host.get_host_port().as_str()
        ),
        MyReverseProxyRemoteEndpoint::OverSsh {
            ssh_credentials,
            remote_host,
        } => format!(
            "{protocol}|ssh:{}|{}",
            ssh_credentials.to_string().as_str(),
            remote_host.get_host_port().as_str()
        ),
        MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
            format!("{protocol}|gw:{}|{}", id, remote_host.get_host_port().as_str())
        }
    }
}
