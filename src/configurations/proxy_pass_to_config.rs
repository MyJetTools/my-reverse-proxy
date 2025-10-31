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
#[derive(Debug)]
pub struct ProxyPassFilesPathModel {
    pub files_path: MyReverseProxyRemoteEndpoint,
    pub default_file: Option<String>,
}

impl ProxyPassFilesPathModel {
    pub fn to_string(&self) -> String {
        self.files_path.to_string()
    }
}

#[derive(Debug)]
pub struct ProxyPassToModel {
    pub remote_host: MyReverseProxyRemoteEndpoint,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub is_mcp: bool,
}

#[derive(Debug)]
pub enum ProxyPassToConfig {
    Http1(ProxyPassToModel),
    Http2(ProxyPassToModel),
    UnixHttp1(ProxyPassToModel),
    UnixHttp2(ProxyPassToModel),
    FilesPath(ProxyPassFilesPathModel),
    Static(Arc<StaticContentConfig>),
}

impl ProxyPassToConfig {
    pub fn to_string(&self) -> String {
        match self {
            ProxyPassToConfig::Http1(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassToConfig::UnixHttp1(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassToConfig::UnixHttp2(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassToConfig::Http2(proxy_pass) => proxy_pass.remote_host.to_string(),

            ProxyPassToConfig::FilesPath(model) => model.to_string(),
            ProxyPassToConfig::Static(model) => model.to_string(),
        }
    }

    pub fn get_type_as_str(&self) -> &'static str {
        match self {
            Self::UnixHttp1(_) => "unix+http1",
            Self::UnixHttp2(_) => "unix+http2",
            Self::Http1(_) => "http1",
            Self::Http2(_) => "http2",
            Self::FilesPath(_) => "files_path",
            Self::Static(_) => crate::consts::location_type::STATIC,
        }
    }
}
