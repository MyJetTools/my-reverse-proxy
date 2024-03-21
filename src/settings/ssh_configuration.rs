use std::sync::Arc;

use my_ssh::{SshCredentials, SshRemoteHost};

#[derive(Debug)]
pub enum SshContent {
    Socket(SshRemoteHost),
    FilePath(String),
}

impl SshContent {
    #[cfg(test)]
    pub fn unwrap_as_socket_addr(&self) -> &SshRemoteHost {
        match self {
            SshContent::Socket(remote_content) => remote_content,
            SshContent::FilePath(file) => {
                panic!("Unwrapping as http but has file: {}", file);
            }
        }
    }
    #[cfg(test)]
    pub fn unwrap_as_file_path(&self) -> &str {
        match self {
            SshContent::Socket(remote_host) => {
                panic!(
                    "Unwrapping as file but has http: {}:{}",
                    remote_host.host, remote_host.port
                );
            }
            SshContent::FilePath(file) => file.as_str(),
        }
    }
}

#[derive(Debug)]
pub struct SshConfiguration {
    pub credentials: Arc<SshCredentials>,
    pub remote_content: SshContent,
}

impl SshConfiguration {
    pub fn parse(src: &str) -> Self {
        let mut parts = src.split("->");
        let ssh_part = parts.next().unwrap();
        let remote_part = parts.next().unwrap();

        let mut ssh_parts = ssh_part.split("@");
        let ssh_user_name = ssh_parts.next().unwrap().split(":").last().unwrap();

        let mut ssh_session_host_port = ssh_parts.next().unwrap().split(":");
        let ssh_session_host = ssh_session_host_port.next().unwrap();

        let ssh_session_port = if let Some(port) = ssh_session_host_port.next() {
            port
        } else {
            "22"
        };

        let remote_content = if remote_part.starts_with("/")
            || remote_part.starts_with("~")
            || remote_part.starts_with(".")
        {
            SshContent::FilePath(remote_part.to_string())
        } else {
            let mut remote_parts = remote_part.split(":");
            let remote_host = remote_parts.next().unwrap();
            let remote_port = remote_parts.last().unwrap();
            SshContent::Socket(SshRemoteHost {
                host: remote_host.to_string(),
                port: remote_port.parse().unwrap(),
            })
        };

        Self {
            credentials: SshCredentials::SshAgent {
                ssh_host_port: SshRemoteHost {
                    host: ssh_session_host.to_string(),
                    port: ssh_session_port.parse().unwrap(),
                },
                ssh_user_name: ssh_user_name.to_string(),
            }
            .into(),

            remote_content: remote_content.into(),
        }
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test_parse_ssh_configuration() {
        let config = "ssh:root@12.12.13.13:22->10.0.0.1:5123";

        let result = super::SshConfiguration::parse(config);

        assert_eq!(result.credentials.get_user_name(), "root");
        assert_eq!(
            result.credentials.get_host_port().to_string(),
            "12.12.13.13:22"
        );

        assert_eq!(
            result.remote_content.unwrap_as_socket_addr().host,
            "10.0.0.1"
        );

        assert_eq!(result.remote_content.unwrap_as_socket_addr().port, 5123);
    }

    #[test]
    fn test_parse_ssh_configuration_as_file() {
        let config = "ssh:root@12.12.13.13:22->/home/user/file.txt";

        let result = super::SshConfiguration::parse(config);

        assert_eq!(result.credentials.get_user_name(), "root");
        assert_eq!(
            result.credentials.get_host_port().to_string(),
            "12.12.13.13:22"
        );

        assert_eq!(
            result.remote_content.unwrap_as_file_path(),
            "/home/user/file.txt"
        );
    }
}
