use std::{collections::HashMap, str::FromStr};

use rust_extensions::StrOrString;

use super::{LocalFilePath, RemoteHost, SshConfigSettings, SshConfiguration};

pub enum ProxyPassTo {
    Http(RemoteHost),
    LocalPath(LocalFilePath),
    Ssh(SshConfiguration),
    Tcp(std::net::SocketAddr),
    Static,
}

impl ProxyPassTo {
    pub fn from_str(
        src: StrOrString<'_>,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
    ) -> Result<Self, String> {
        if src.as_str().trim() == "static" {
            return Ok(ProxyPassTo::Static);
        }

        if src.as_str().starts_with("ssh") {
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
            return Ok(ProxyPassTo::LocalPath(LocalFilePath::new(src.to_string())));
        }

        Ok(ProxyPassTo::Tcp(
            std::net::SocketAddr::from_str(src.as_str()).unwrap(),
        ))
    }
}
