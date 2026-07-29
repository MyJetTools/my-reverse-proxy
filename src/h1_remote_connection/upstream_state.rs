use crate::configurations::{MyReverseProxyRemoteEndpoint, ProxyPassToConfig, ProxyPassToModel};

/// The upstream path an MCP location rewrites requests onto, if this is one.
/// Depends only on the config, not on the chosen connection.
pub fn mcp_path(proxy_pass_to: &ProxyPassToConfig) -> Option<&str> {
    match proxy_pass_to {
        ProxyPassToConfig::McpHttp1(model) | ProxyPassToConfig::McpHttp2(model) => {
            Some(model.remote_host.get_path_and_query())
        }
        _ => None,
    }
}

/// Identity of an upstream connection: remote host + protocol. Used as the pool
/// key — two locations with the same key share the upstream connection slot.
///
/// MCP is the one case where the PATH is part of that identity. An ordinary
/// location forwards the client's own path, so the upstream path plays no role
/// in what a connection is; an MCP location rewrites every request onto the
/// path configured in `proxy_pass_to`, which is what names the MCP server. Two
/// MCP servers published on one `host:port` under different paths are different
/// upstreams and get different pools.
pub fn connection_key(proxy_pass_to: &ProxyPassToConfig) -> String {
    match proxy_pass_to {
        ProxyPassToConfig::Http1(model) => remote_host_key("h1", model),
        ProxyPassToConfig::McpHttp1(model) => mcp_key("mcp-h1", model),
        ProxyPassToConfig::McpHttp2(model) => mcp_key("mcp-h2", model),
        ProxyPassToConfig::Http2(model) => remote_host_key("h2", model),
        ProxyPassToConfig::UnixHttp1(model) => remote_host_key("uds-h1", model),
        ProxyPassToConfig::UnixHttp2(model) => remote_host_key("uds-h2", model),
        other => other.to_string(),
    }
}

fn mcp_key(protocol: &str, model: &ProxyPassToModel) -> String {
    format!(
        "{}|{}",
        remote_host_key(protocol, model),
        model.remote_host.get_path_and_query()
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rust_extensions::remote_endpoint::RemoteEndpointOwned;

    use super::*;

    fn model(url: &str) -> ProxyPassToModel {
        ProxyPassToModel {
            remote_host: MyReverseProxyRemoteEndpoint::Direct {
                remote_host: Arc::new(RemoteEndpointOwned::try_parse(url.to_string()).unwrap()),
            },
            request_timeout: std::time::Duration::from_secs(1),
            connect_timeout: std::time::Duration::from_secs(1),
            pool_tuning: crate::configurations::PoolTuning::default(),
        }
    }

    /// The bug this key shape exists for: several MCP servers published on one
    /// `host:port` under different paths are different upstreams. Sharing a
    /// connection between them sends a request meant for one server to another,
    /// because the path is rewritten per request.
    #[test]
    fn mcp_locations_differing_only_by_path_are_different_upstreams() {
        let a = connection_key(&ProxyPassToConfig::McpHttp1(model(
            "http://10.0.20.166:8007/mcp-a",
        )));
        let b = connection_key(&ProxyPassToConfig::McpHttp1(model(
            "http://10.0.20.166:8007/mcp-b",
        )));

        assert_ne!(a, b);
    }

    /// An ordinary http1 location forwards the client's own path, so the same
    /// `host:port` is one upstream no matter which location reached it.
    #[test]
    fn http1_locations_differing_only_by_path_share_the_upstream() {
        let a = connection_key(&ProxyPassToConfig::Http1(model("http://host:8000/one")));
        let b = connection_key(&ProxyPassToConfig::Http1(model("http://host:8000/two")));

        assert_eq!(a, b);
    }

    /// mcp / mcp-h2 / http1 to the same host:port are not interchangeable
    /// either — the protocol spoken on the wire (and, for mcp, the rewritten
    /// path) differs.
    #[test]
    fn the_protocol_is_part_of_the_key() {
        let url = "http://host:8000/mcp";
        let h1 = connection_key(&ProxyPassToConfig::Http1(model(url)));
        let mcp_h1 = connection_key(&ProxyPassToConfig::McpHttp1(model(url)));
        let mcp_h2 = connection_key(&ProxyPassToConfig::McpHttp2(model(url)));

        assert_ne!(h1, mcp_h1);
        assert_ne!(mcp_h1, mcp_h2);
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
            format!(
                "{protocol}|gw:{}|{}",
                id,
                remote_host.get_host_port().as_str()
            )
        }
    }
}
