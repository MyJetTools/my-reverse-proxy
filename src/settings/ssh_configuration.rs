use std::sync::Arc;

use my_ssh::SshCredentials;
use rust_extensions::StrOrString;

use super::RemoteHost;

#[derive(Debug)]
pub enum SshContent {
    RemoteHost(RemoteHost),
    FilePath(String),
}

impl SshContent {
    #[cfg(test)]
    pub fn unwrap_as_remote_host(&self) -> &RemoteHost {
        match self {
            SshContent::RemoteHost(remote_host) => remote_host,
            SshContent::FilePath(file) => {
                panic!("Ssh remote content must be not a local file {}", file);
            }
        }
    }
    #[cfg(test)]
    pub fn unwrap_as_file_path(&self) -> &str {
        match self {
            SshContent::RemoteHost(remote_host) => {
                panic!("Unwrapping as file but has http: {}", remote_host.as_str());
            }
            SshContent::FilePath(file) => file.as_str(),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            SshContent::RemoteHost(remote_host) => remote_host.as_str(),
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
    pub fn parse(src: StrOrString) -> Self {
        let mut parts = src.as_str().split("->");
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
            SshContent::RemoteHost(remote_part.to_string().into())
        };

        Self {
            credentials: SshCredentials::SshAgent {
                ssh_remote_host: ssh_session_host.to_string(),
                ssh_remote_port: ssh_session_port.parse().unwrap(),
                ssh_user_name: ssh_user_name.to_string(),
            }
            .into(),

            remote_content: remote_content.into(),
        }
    }

    /*
    pub fn to_string(&self) -> String {
        let (h, p) = self.credentials.get_host_port();
        format!(
            "ssh:{}@{}:{}->{}",
            self.credentials.get_user_name(),
            h,
            p,
            self.remote_content.as_str()
        )
    }
     */
}

#[cfg(test)]
mod test {

    #[test]
    fn test_parse_ssh_configuration() {
        let config = "ssh:root@12.12.13.13:22->10.0.0.1:5123";

        let result = super::SshConfiguration::parse(config.into());

        assert_eq!(result.credentials.get_user_name(), "root");
        assert_eq!(
            result.credentials.get_host_port_as_string(),
            "12.12.13.13:22"
        );

        assert_eq!(
            result.remote_content.unwrap_as_remote_host().as_str(),
            "10.0.0.1:5123"
        );
    }

    #[test]
    fn test_parse_ssh_configuration_as_file() {
        let config = "ssh:root@12.12.13.13:22->/home/user/file.txt";

        let result = super::SshConfiguration::parse(config.into());

        assert_eq!(result.credentials.get_user_name(), "root");
        assert_eq!(
            result.credentials.get_host_port_as_string(),
            "12.12.13.13:22"
        );

        assert_eq!(
            result.remote_content.unwrap_as_file_path(),
            "/home/user/file.txt"
        );
    }
}
