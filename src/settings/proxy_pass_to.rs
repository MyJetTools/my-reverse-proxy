use std::time::Duration;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::configurations::MyReverseProxyRemoteEndpoint;

pub struct StaticContentModel {
    pub status_code: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl StaticContentModel {
    pub fn to_string(&self) -> String {
        format!(
            "status_code: {}, content_type: {:?}, body: {}bytes",
            self.status_code,
            self.content_type,
            self.body.len()
        )
    }
}

pub struct ProxyPassFilesPathModel {
    pub files_path: MyReverseProxyRemoteEndpoint,
    pub default_file: Option<String>,
}

impl ProxyPassFilesPathModel {
    pub fn to_string(&self) -> String {
        self.files_path.to_string()
    }
}

pub struct ProxyPassToModel {
    pub remote_host: OverSshConnectionSettings,
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
}

pub enum ProxyPassTo {
    Http1(ProxyPassToModel),
    Http2(ProxyPassToModel),
    FilesPath(ProxyPassFilesPathModel),
    Static(StaticContentModel),
}

impl ProxyPassTo {
    pub fn to_string(&self) -> String {
        match self {
            ProxyPassTo::Http1(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassTo::Http2(proxy_pass) => proxy_pass.remote_host.to_string(),
            ProxyPassTo::FilesPath(model) => model.to_string(),
            ProxyPassTo::Static(model) => model.to_string(),
        }
    }

    pub fn get_type_as_str(&self) -> &'static str {
        match self {
            ProxyPassTo::Http1(_) => "http1",
            ProxyPassTo::Http2(_) => "http2",
            ProxyPassTo::FilesPath(_) => "files_path",
            ProxyPassTo::Static(_) => "static",
        }
    }
}
