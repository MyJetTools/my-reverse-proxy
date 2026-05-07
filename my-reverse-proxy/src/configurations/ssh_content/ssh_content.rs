use std::sync::Arc;

use my_ssh::SshCredentials;

use super::*;

#[derive(Debug, Clone)]
pub struct SshContentSource {
    pub credentials: Arc<SshCredentials>,
    pub remote_content: SshRemoteContent,
}

impl SshContentSource {
    pub fn parse(ssh_part: &str, remote_part: &str) -> Result<Self, String> {
        let remote_content = SshRemoteContent::parse(remote_part)?;

        let (user_name, host, port) = parse_ssh_part(ssh_part);

        let result = Self {
            remote_content,
            credentials: SshCredentials::SshAgent {
                ssh_remote_host: host.to_string(),
                ssh_remote_port: port,
                ssh_user_name: user_name.to_string(),
            }
            .into(),
        };

        Ok(result)
    }

    pub fn to_string(&self) -> String {
        format!(
            "{}@{}->{}",
            self.credentials.get_user_name(),
            self.credentials.get_host_port_as_string(),
            self.remote_content.as_str()
        )
    }
}

fn parse_ssh_part(ssh_part: &str) -> (&str, &str, u16) {
    let mut ssh_parts = ssh_part.split("@");
    let ssh_user_name = ssh_parts.next().unwrap().split(":").last().unwrap();

    let mut ssh_session_host_port = ssh_parts.next().unwrap().split(":");
    let ssh_session_host = ssh_session_host_port.next().unwrap();

    let ssh_session_port = if let Some(port) = ssh_session_host_port.next() {
        port
    } else {
        "22"
    };

    (
        ssh_user_name,
        ssh_session_host,
        ssh_session_port.parse().unwrap(),
    )
}

#[cfg(test)]
mod test {

    #[test]
    fn test_parse_ssh_configuration() {
        let result =
            super::SshContentSource::parse("ssh:root@12.12.13.13:22", "10.0.0.1:5123").unwrap();

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
        let result =
            super::SshContentSource::parse("ssh:root@12.12.13.13:22", "/home/user/file.txt")
                .unwrap();

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
