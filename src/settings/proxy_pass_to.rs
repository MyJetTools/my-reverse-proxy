use std::str::FromStr;

use hyper::Uri;

use super::{ContentSourceSettings, FileName, HttpProxyPassRemoteEndpoint, SshConfiguration};

pub struct ProxyPassTo(String);

impl ProxyPassTo {
    pub fn new(location: String) -> Self {
        Self(location)
    }

    pub fn is_ssh(&self) -> bool {
        self.0.starts_with("ssh")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_ssh_configuration(&self) -> Option<SshConfiguration> {
        if self.is_ssh() {
            return Some(SshConfiguration::parse(self.as_str()));
        }

        None
    }

    pub fn to_content_source(
        &self,
        is_http1: bool,
        default_file: Option<String>,
    ) -> ContentSourceSettings {
        if let Some(ssh_configuration) = self.to_ssh_configuration() {
            match ssh_configuration.remote_content {
                super::SshContent::Socket(ssh_remote_host) => {
                    let result = if is_http1 {
                        HttpProxyPassRemoteEndpoint::Http1OverSsh {
                            ssh_credentials: ssh_configuration.credentials,
                            remote_host: ssh_remote_host,
                        }
                    } else {
                        HttpProxyPassRemoteEndpoint::Http2OverSsh {
                            ssh_credentials: ssh_configuration.credentials,
                            remote_host: ssh_remote_host,
                        }
                    };

                    return ContentSourceSettings::Http(result);
                }
                super::SshContent::FilePath(file_path) => {
                    return ContentSourceSettings::FileOverSsh {
                        ssh_credentials: ssh_configuration.credentials,
                        file_path,
                        default_file,
                    };
                }
            }
        } else {
            if self.as_str().starts_with("http") {
                let result = if is_http1 {
                    HttpProxyPassRemoteEndpoint::Http(Uri::from_str(self.as_str()).unwrap())
                } else {
                    HttpProxyPassRemoteEndpoint::Http2(Uri::from_str(self.as_str()).unwrap())
                };

                return ContentSourceSettings::Http(result);
            }

            ContentSourceSettings::File {
                file_name: FileName::new(self.as_str()),
                default_file,
            }
        }
    }
}
