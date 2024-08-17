use std::{collections::HashMap, time::Duration};

use my_settings_reader::flurl::FlUrl;
use my_ssh::SshSession;
use rust_extensions::StrOrString;

use crate::{files_cache::FilesCache, variables_reader::VariablesReader};

use super::*;

pub enum FileSource {
    File(String),
    Http(String),
    Ssh(SshConfiguration),
}

impl FileSource {
    pub fn from_src(
        src: StrOrString,
        ssh_config: &Option<HashMap<String, SshConfigSettings>>,
        variables: VariablesReader,
    ) -> Result<Self, String> {
        if src.as_str().starts_with("http") {
            return Ok(FileSource::Http(src.to_string()));
        }

        if src.as_str().starts_with(super::SSH_PREFIX) {
            return Ok(FileSource::Ssh(SshConfiguration::parse(
                src.as_str(),
                ssh_config,
                variables,
            )?));
        }

        Ok(Self::File(src.to_string()))
    }

    pub fn as_str<'s>(&'s self) -> StrOrString<'s> {
        match self {
            FileSource::File(s) => s.into(),
            FileSource::Http(s) => s.into(),
            FileSource::Ssh(s) => format!(
                "{}->{}",
                s.credentials.to_string(),
                s.remote_content.as_str()
            )
            .into(),
        }
    }

    pub async fn load_file_content(&self, cache: Option<&FilesCache>) -> Result<Vec<u8>, String> {
        match self {
            FileSource::File(file_name) => {
                println!("Loading file {}", file_name);
                let file_name = LocalFilePath::new(file_name.to_string());

                let result = tokio::fs::read(file_name.get_value().as_str())
                    .await
                    .map_err(|err| {
                        format!(
                            "Error reading file: {:?}, error: {:?}",
                            file_name.get_value().as_str(),
                            err
                        )
                    })?;

                Ok(result)
            }
            FileSource::Http(url) => {
                if let Some(cache) = cache {
                    if let Some(value) = cache.get(url).await {
                        return Ok(value);
                    }
                }

                let response = FlUrl::new(url)
                    .get()
                    .await
                    .map_err(|err| format!("Error loading file from HTTP. Error: {:?}", err))?;

                let result = response
                    .receive_body()
                    .await
                    .map_err(|itm| format!("Error loading file from HTTP. Error: {:?}", itm))?;

                if let Some(cache) = cache {
                    cache.add(url.to_string(), result.clone()).await;
                }

                Ok(result)
            }
            FileSource::Ssh(ssh_credentials) => match &ssh_credentials.remote_content {
                SshContent::RemoteHost(_) => {
                    panic!("Reading file is not supported from socket yet");
                }
                SshContent::FilePath(path) => {
                    let ssh_cred_as_string = ssh_credentials.to_string();

                    if let Some(cache) = cache {
                        if let Some(value) = cache.get(ssh_cred_as_string.as_str()).await {
                            return Ok(value);
                        }
                    }

                    println!(
                        "Loading file from remove resource using SSH. {}->{}",
                        ssh_credentials.credentials.to_string(),
                        path
                    );
                    let ssh_session = SshSession::new(ssh_credentials.credentials.clone().into());

                    let result = ssh_session
                        .download_remote_file(&path, Duration::from_secs(5))
                        .await;

                    if let Err(err) = result {
                        match ssh_credentials.credentials.as_ref() {
                            my_ssh::SshCredentials::SshAgent {
                                ssh_remote_host,
                                ssh_remote_port,
                                ssh_user_name,
                            } => {
                                println!(
                                    "SSH Agent: {}:{}@{}",
                                    ssh_user_name, ssh_remote_port, ssh_remote_host
                                )
                            }
                            my_ssh::SshCredentials::UserNameAndPassword {
                                ssh_remote_host,
                                ssh_remote_port,
                                ssh_user_name,
                                password: _,
                            } => {
                                println!(
                                    "SSH User: {}:{}@{}",
                                    ssh_user_name, ssh_remote_port, ssh_remote_host
                                )
                            }
                            my_ssh::SshCredentials::PrivateKey {
                                ssh_remote_host,
                                ssh_remote_port,
                                ssh_user_name,
                                private_key: _,
                                passphrase: _,
                            } => {
                                println!(
                                    "SSH Private Key: {}:{}@{}",
                                    ssh_user_name, ssh_remote_port, ssh_remote_host
                                )
                            }
                        }

                        return Err(format!(
                            "Can not download file from remote resource. Error: {:?}",
                            err
                        ));
                    }

                    let result = result.unwrap();

                    if let Some(cache) = cache {
                        cache.add(ssh_cred_as_string, result.clone()).await;
                    }

                    Ok(result)
                }
            },
        }
    }
}
