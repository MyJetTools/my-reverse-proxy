use super::{LocalFilePath, RemoteHost, SshConfiguration};

pub struct StaticContentModel {
    pub status_code: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

pub struct LocalPathModel {
    pub local_path: LocalFilePath,
    pub default_file: Option<String>,
}

pub struct SshProxyPassModel {
    pub ssh_config: SshConfiguration,
    pub http2: bool,
    pub default_file: Option<String>,
}

pub enum ProxyPassTo {
    Http(RemoteHost),
    Http2(RemoteHost),
    LocalPath(LocalPathModel),
    Ssh(SshProxyPassModel),
    Tcp(std::net::SocketAddr),
    Static(StaticContentModel),
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
