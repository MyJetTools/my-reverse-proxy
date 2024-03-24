use std::str::FromStr;

use rust_extensions::StrOrString;

use super::{LocalFilePath, RemoteHost, SshConfiguration};

pub enum ProxyPassTo {
    Http(RemoteHost),
    LocalPath(LocalFilePath),
    Ssh(SshConfiguration),
    Tcp(std::net::SocketAddr),
    Static,
}

impl ProxyPassTo {
    pub fn from_str(src: StrOrString<'_>) -> Self {
        if src.as_str().trim() == "static" {
            return ProxyPassTo::Static;
        }

        if src.as_str().starts_with("ssh") {
            return ProxyPassTo::Ssh(SshConfiguration::parse(src));
        }

        if src.as_str().starts_with("http") {
            return ProxyPassTo::Http(RemoteHost::new(src.to_string()));
        }

        if src.as_str().starts_with("~")
            || src.as_str().starts_with("/")
            || src.as_str().starts_with(".")
        {
            return ProxyPassTo::LocalPath(LocalFilePath::new(src.to_string()));
        }

        ProxyPassTo::Tcp(std::net::SocketAddr::from_str(src.as_str()).unwrap())
    }
}
