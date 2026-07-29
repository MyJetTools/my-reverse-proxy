use std::{sync::Arc, time::Duration};

use crate::configurations::MyReverseProxyRemoteEndpoint;

#[derive(Debug, Clone)]
pub struct StaticContentConfig {
    pub status_code: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl StaticContentConfig {
    pub fn to_string(&self) -> String {
        format!(
            "status_code: {}, content_type: {:?}, body: {}bytes",
            self.status_code,
            self.content_type,
            self.body.len()
        )
    }
}
#[derive(Debug, Clone)]
pub struct ProxyPassFilesPathModel {
    pub files_path: MyReverseProxyRemoteEndpoint,
    pub default_file: Option<String>,
}

impl ProxyPassFilesPathModel {
    pub fn to_string(&self) -> String {
        self.files_path.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct ProxyPassToModel {
    pub remote_host: MyReverseProxyRemoteEndpoint,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub pool_tuning: super::PoolTuning,
}

#[derive(Debug, Clone)]
pub struct DynamicProxyConfig {
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub allowed_hosts: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub enum ProxyPassToConfig {
    Http1(ProxyPassToModel),
    Http2(ProxyPassToModel),
    /// MCP over an HTTP/1 upstream — an `Http1` upstream in every respect that
    /// concerns connecting and pooling; see [`ProxyPassToConfig::is_mcp`] for
    /// what the marker actually changes.
    McpHttp1(ProxyPassToModel),
    /// MCP over an HTTP/2 upstream. Served by the hyper path only (the h1 byte
    /// pipeline has no h2 upstream), so it needs an `http2`/`https2` endpoint.
    McpHttp2(ProxyPassToModel),
    UnixHttp1(ProxyPassToModel),
    UnixHttp2(ProxyPassToModel),
    FilesPath(ProxyPassFilesPathModel),
    Static(Arc<StaticContentConfig>),
    Drop,
    DynamicProxy(Arc<DynamicProxyConfig>),
}

impl ProxyPassToConfig {
    pub fn to_string(&self) -> String {
        match self {
            ProxyPassToConfig::Http1(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassToConfig::McpHttp1(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassToConfig::McpHttp2(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassToConfig::UnixHttp1(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassToConfig::UnixHttp2(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassToConfig::Http2(proxy_pass) => proxy_pass.remote_host.to_string(),

            ProxyPassToConfig::FilesPath(model) => model.to_string(),
            ProxyPassToConfig::Static(model) => model.to_string(),
            ProxyPassToConfig::Drop => "drop".to_string(),
            ProxyPassToConfig::DynamicProxy(_) => crate::consts::location_type::DYNAMIC.to_string(),
        }
    }

    pub fn get_type_as_str(&self) -> &'static str {
        match self {
            Self::UnixHttp1(_) => "unix+http1",
            Self::UnixHttp2(_) => "unix+http2",
            Self::Http1(_) => "http1",
            Self::McpHttp1(_) => crate::consts::location_type::MCP,
            Self::McpHttp2(_) => crate::consts::location_type::MCP_H2,
            Self::Http2(_) => "http2",
            Self::FilesPath(_) => "files_path",
            Self::Static(_) => crate::consts::location_type::STATIC,
            Self::Drop => "drop",
            Self::DynamicProxy(_) => crate::consts::location_type::DYNAMIC,
        }
    }

    pub fn is_drop(&self) -> bool {
        matches!(self, Self::Drop)
    }

    /// An MCP location. Connecting and pooling are exactly an `Http1` / `Http2`
    /// upstream's — the marker changes only three things: the request path is
    /// rewritten onto the configured upstream path
    /// ([`crate::h1_remote_connection::mcp_path`]), the
    /// read timeout is the long MCP one (an SSE stream idles for hours), and a
    /// failed upstream drops the client connection instead of substituting an
    /// HTML error page a JSON-RPC client cannot read.
    pub fn is_mcp(&self) -> bool {
        matches!(self, Self::McpHttp1(_) | Self::McpHttp2(_))
    }

    /// Returns the upstream's transport kind (`direct` / `ssh` / `gateway`)
    /// for variants that carry a `MyReverseProxyRemoteEndpoint`. `None` for
    /// static / drop variants that don't reach a remote.
    pub fn remote_endpoint_kind(&self) -> Option<&'static str> {
        match self {
            Self::Http1(m)
            | Self::McpHttp1(m)
            | Self::McpHttp2(m)
            | Self::Http2(m)
            | Self::UnixHttp1(m)
            | Self::UnixHttp2(m) => Some(m.remote_host.kind_as_str()),
            Self::FilesPath(m) => Some(m.files_path.kind_as_str()),
            Self::Static(_) | Self::Drop | Self::DynamicProxy(_) => None,
        }
    }
}
