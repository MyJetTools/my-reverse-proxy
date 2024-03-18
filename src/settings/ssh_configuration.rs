use std::fmt::format;

use my_ssh::SshCredentials;

#[derive(Debug)]
pub struct SshConfiguration {
    pub ssh_user_name: String,
    pub ssh_session_host: String,
    pub ssh_session_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

impl SshConfiguration {
    pub fn to_string(&self) -> String {
        format!(
            "ssh:{}@{}:{}->{}:{}",
            self.ssh_user_name,
            self.ssh_session_host,
            self.ssh_session_port,
            self.remote_host,
            self.remote_port
        )
    }

    pub fn to_ssh_credentials(&self) -> SshCredentials {
        SshCredentials::SshAgent {
            ssh_host_port: format!("{}:{}", self.ssh_session_host, self.ssh_session_port),
            ssh_user_name: self.ssh_user_name.to_string(),
        }
    }

    pub fn parse(src: &str) -> Self {
        let mut parts = src.split("->");
        let ssh_part = parts.next().unwrap();
        let remote_part = parts.next().unwrap();

        let mut ssh_parts = ssh_part.split("@");
        let ssh_user_name = ssh_parts.next().unwrap().split(":").last().unwrap();

        let ssh_session_host = ssh_parts.next().unwrap();
        let ssh_session_port = ssh_session_host.split(":").last().unwrap().parse().unwrap();
        let ssh_session_host = ssh_session_host.split(":").next().unwrap();

        let mut remote_parts = remote_part.split(":");
        let remote_host = remote_parts.next().unwrap();
        let remote_port = remote_parts.last().unwrap().parse().unwrap();

        Self {
            ssh_user_name: ssh_user_name.to_string(),
            ssh_session_host: ssh_session_host.to_string(),
            ssh_session_port,
            remote_host: remote_host.to_string(),
            remote_port,
        }
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test_parse_ssh_configuration() {
        let config = "ssh:root@12.12.13.13:22->10.0.0.1:5123";

        let result = super::SshConfiguration::parse(config);

        assert_eq!(result.ssh_user_name, "root");
        assert_eq!(result.ssh_session_host, "12.12.13.13");
        assert_eq!(result.ssh_session_port, 22);

        assert_eq!(result.remote_host, "10.0.0.1");
        assert_eq!(result.remote_port, 5123);
    }
}
