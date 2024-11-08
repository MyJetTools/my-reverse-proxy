use std::{collections::HashMap, sync::Arc};

use my_ssh::SshCredentials;

use crate::variables_reader::VariablesReader;

use super::{LocalFilePath, RemoteHost, SshConfigSettings};

pub const SSH_PREFIX: &str = "ssh:";

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct SshConfiguration {
    pub credentials: Arc<SshCredentials>,
    pub remote_content: SshContent,
}

impl SshConfiguration {
    pub fn parse(
        src: &str,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
        variables_reader: VariablesReader,
    ) -> Result<Self, String> {
        let mut parts = src.split("->");
        let ssh_part = parts.next().unwrap();

        let ssh_part = ssh_part[SSH_PREFIX.len()..].trim();

        let remote_part = parts.next().unwrap();

        let remote_content = parse_remote_part(remote_part);

        if let Some(ssh_configs) = ssh_configs {
            if let Some(ssh_config_settings) = ssh_configs.get(ssh_part) {
                match ssh_config_settings.get_option(variables_reader)? {
                    super::SshConfigOption::AsPassword(password) => {
                        let (ssh_user_name, ssh_session_host, ssh_session_port) =
                            parse_ssh_part(ssh_part);
                        println!("SSH: {} using Login+Password for authentication", ssh_part);
                        return Ok(Self {
                            credentials: SshCredentials::UserNameAndPassword {
                                ssh_remote_host: ssh_session_host.to_string(),
                                ssh_remote_port: ssh_session_port,
                                ssh_user_name: ssh_user_name.to_string(),
                                password,
                            }
                            .into(),
                            remote_content,
                        });
                    }
                    super::SshConfigOption::AsPrivateKeyFile {
                        file_path,
                        passphrase,
                    } => {
                        let (ssh_user_name, ssh_session_host, ssh_session_port) =
                            parse_ssh_part(ssh_part);
                        println!("SSH: {} using PrivateKey for authentication", ssh_part);
                        return Ok(Self {
                            credentials: SshCredentials::PrivateKey {
                                ssh_remote_host: ssh_session_host.to_string(),
                                ssh_remote_port: ssh_session_port,
                                ssh_user_name: ssh_user_name.to_string(),
                                private_key: load_private_key(&file_path)?,
                                passphrase,
                            }
                            .into(),
                            remote_content,
                        });
                    }
                }
            } else {
                let (ssh_user_name, ssh_session_host, ssh_session_port) = parse_ssh_part(ssh_part);
                println!("SSH: {} using SshAgent for authentication", ssh_part);
                let result = Self {
                    credentials: SshCredentials::SshAgent {
                        ssh_remote_host: ssh_session_host.to_string(),
                        ssh_remote_port: ssh_session_port,
                        ssh_user_name: ssh_user_name.to_string(),
                    }
                    .into(),
                    remote_content,
                };

                Ok(result)
            }
        } else {
            let (ssh_user_name, ssh_session_host, ssh_session_port) = parse_ssh_part(ssh_part);
            println!("SSH: {} using SshAgent for authentication", ssh_part);

            let result = Self {
                credentials: SshCredentials::SshAgent {
                    ssh_remote_host: ssh_session_host.to_string(),
                    ssh_remote_port: ssh_session_port,
                    ssh_user_name: ssh_user_name.to_string(),
                }
                .into(),
                remote_content: parse_remote_part(remote_part),
            };

            Ok(result)
        }
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

fn parse_remote_part(remote_part: &str) -> SshContent {
    if remote_part.starts_with("/") || remote_part.starts_with("~") || remote_part.starts_with(".")
    {
        SshContent::FilePath(remote_part.to_string())
    } else {
        SshContent::RemoteHost(remote_part.to_string().into())
    }
}

fn load_private_key(file_path: &LocalFilePath) -> Result<String, String> {
    let file_path = file_path.get_value();
    std::fs::read_to_string(file_path.as_str()).map_err(|err| {
        format!(
            "Can not load ssh private key from {}. Err: {}",
            file_path.as_str(),
            err
        )
    })
}

#[cfg(test)]
mod test {

    #[test]
    fn test_parse_ssh_configuration() {
        let config = "ssh:root@12.12.13.13:22->10.0.0.1:5123";

        let result = super::SshConfiguration::parse(config.into(), &None, (&None).into()).unwrap();

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

        let result = super::SshConfiguration::parse(config.into(), &None, (&None).into()).unwrap();

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
