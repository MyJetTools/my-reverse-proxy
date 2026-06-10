use std::collections::HashMap;

use crate::configurations::{MyReverseProxyRemoteEndpoint, ProxyPassToConfig, ProxyPassToModel};
use crate::network_stream::*;

use super::*;

struct UpstreamEntry {
    upstream: Upstream,
    disposed: bool,
}

/// Pool of upstream connections scoped to one incoming TCP connection.
/// Entries are keyed by remote host + protocol: locations pointing at the same
/// remote share one connection, a location with a different remote gets its
/// own. A failed request marks its entry disposed; the pool finally disposes
/// it and reconnects on the next handout.
pub struct UpstreamState {
    upstreams: HashMap<String, UpstreamEntry>,
}

pub struct UpstreamAccess<'s, 'c> {
    pub upstream: &'s mut Upstream,
    pub mcp_path: Option<&'c str>,
}

impl UpstreamState {
    pub fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub async fn get_or_connect<
        's,
        'c,
        WritePart: NetworkStreamWritePart + Send + Sync + 'static,
        ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    >(
        &'s mut self,
        proxy_pass_to: &'c ProxyPassToConfig,
        ctx: &Http1ServerConnectionContext<WritePart, ReadPart>,
    ) -> Result<UpstreamAccess<'s, 'c>, NetworkError> {
        let key = connection_key(proxy_pass_to);

        // An entry marked disposed by a failed request, or one whose remote
        // end already closed, is finally dropped here and recreated below.
        if let Some(entry) = self.upstreams.get(&key) {
            if entry.disposed || entry.upstream.is_disconnected() {
                self.upstreams.remove(&key);
            }
        }

        if !self.upstreams.contains_key(&key) {
            let upstream = Upstream::connect(proxy_pass_to, ctx).await?;
            self.upstreams.insert(
                key.clone(),
                UpstreamEntry {
                    upstream,
                    disposed: false,
                },
            );
        }

        let mcp_path = match proxy_pass_to {
            ProxyPassToConfig::McpHttp1(model) => Some(model.remote_host.get_path_and_query()),
            _ => None,
        };

        let entry = self.upstreams.get_mut(&key).unwrap();

        Ok(UpstreamAccess {
            upstream: &mut entry.upstream,
            mcp_path,
        })
    }

    /// The request on this upstream failed mid-flight. The connection may be
    /// desynced, so it must not serve another request — but it is not dropped
    /// here: its response_read_loop may still be pumping. The next
    /// get_or_connect for the same remote disposes it for real.
    pub fn mark_disposed(&mut self, proxy_pass_to: &ProxyPassToConfig) {
        if let Some(entry) = self.upstreams.get_mut(&connection_key(proxy_pass_to)) {
            entry.disposed = true;
        }
    }

    pub fn take(&mut self, key: &str) -> Option<Upstream> {
        self.upstreams.remove(key).map(|entry| entry.upstream)
    }
}

/// Identity of an upstream connection: remote host + protocol. Two locations
/// with the same key can share one connection within the incoming connection.
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
        MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => format!(
            "{protocol}|gw:{}|{}",
            id,
            remote_host.get_host_port().as_str()
        ),
    }
}
