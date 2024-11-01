use crate::configurations::*;

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

pub struct LocalPathModel {
    pub local_path: LocalFilePath,
    pub default_file: Option<String>,
}

impl LocalPathModel {
    pub fn to_string(&self) -> String {
        self.local_path.get_value().to_string()
    }
}

pub struct SshProxyPassModel {
    pub ssh_config: SshConfiguration,
    pub http2: bool,
    pub default_file: Option<String>,
}

impl SshProxyPassModel {
    pub fn to_string(&self) -> String {
        self.ssh_config.to_string()
    }
}

pub enum ProxyPassTo {
    Http1(RemoteHost),
    Http2(RemoteHost),
    LocalPath(LocalPathModel),
    Ssh(SshProxyPassModel),
    Tcp(std::net::SocketAddr),
    Static(StaticContentModel),
}

impl ProxyPassTo {
    pub fn to_string(&self) -> String {
        match self {
            ProxyPassTo::Http1(remote_host) => remote_host.to_string(),
            ProxyPassTo::Http2(remote_host) => remote_host.to_string(),
            ProxyPassTo::LocalPath(model) => model.to_string(),
            ProxyPassTo::Ssh(model) => model.to_string(),
            ProxyPassTo::Tcp(socket_addr) => format!("{}", socket_addr),
            ProxyPassTo::Static(model) => model.to_string(),
        }
    }
}

/*
impl ProxyPassTo {
    pub fn from_str(
        src: StrOrString<'_>,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
        get_static_content_model: impl FnOnce() -> Result<StaticContentModel, String>,
    ) -> Result<Self, String> {
        if src.as_str().trim() == "static" {
            return Ok(ProxyPassTo::Static(get_static_content_model()?));
        }

        if src.as_str().starts_with(super::SSH_PREFIX) {
            return Ok(ProxyPassTo::Ssh(SshConfiguration::parse(
                src,
                &ssh_configs,
            )?));
        }

        if src.as_str().starts_with("http") {
            return Ok(ProxyPassTo::Http(RemoteHost::new(src.to_string())));
        }

        if src.as_str().starts_with("~")
            || src.as_str().starts_with("/")
            || src.as_str().starts_with(".")
        {
            return Ok(ProxyPassTo::LocalPath(LocalPathModel {
                local_path: LocalFilePath::new(src.to_string()),
                default_file: self.,
            }));
        }

        Ok(ProxyPassTo::Tcp(
            std::net::SocketAddr::from_str(src.as_str()).unwrap(),
        ))
    }
}
 */
