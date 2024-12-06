use std::{collections::HashMap, sync::Arc, time::Duration};

use http::Method;
use http_body_util::BodyExt;
use my_http_client::http1::{MyHttpClient, MyHttpRequestBuilder};
use my_settings_reader::flurl::FlUrl;
use my_ssh::{SshCredentials, SshSession};
use rust_extensions::StrOrString;

use crate::{files_cache::FilesCache, variables_reader::VariablesReader};

use super::*;

#[derive(Debug, Clone)]
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

    pub async fn load_file_content(
        &self,
        cache: Option<&FilesCache>,
        init_on_start: bool,
    ) -> Result<Vec<u8>, String> {
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
            FileSource::Ssh(ssh_configuration) => match &ssh_configuration.remote_content {
                SshContent::RemoteHost(host) => {
                    if !rust_extensions::str_utils::starts_with_case_insensitive(
                        host.as_str(),
                        "http",
                    ) {
                        panic!("Reading content from remote source supports only by http");
                    }

                    let ssh_cred_as_string = ssh_configuration.to_string();

                    if let Some(cache) = cache {
                        if let Some(value) = cache.get(ssh_cred_as_string.as_str()).await {
                            return Ok(value);
                        }
                    }

                    loop {
                        match loading_content_from_http_via_ssh(ssh_configuration, host).await {
                            Ok(result) => {
                                if let Some(cache) = cache {
                                    cache.add(ssh_cred_as_string, result.clone()).await;
                                }

                                return Ok(result);
                            }
                            Err(err) => {
                                if !init_on_start {
                                    return Err(err);
                                }

                                println!("Can not load file from SSH. Error: {}. Retrying...", err);
                                tokio::time::sleep(Duration::from_secs(3)).await;
                            }
                        }
                    }
                }
                SshContent::FilePath(path) => {
                    let ssh_cred_as_string = ssh_configuration.to_string();

                    if let Some(cache) = cache {
                        if let Some(value) = cache.get(ssh_cred_as_string.as_str()).await {
                            return Ok(value);
                        }
                    }

                    loop {
                        match loading_file_from_ssh(ssh_configuration, path).await {
                            Ok(result) => {
                                if let Some(cache) = cache {
                                    cache.add(ssh_cred_as_string, result.clone()).await;
                                }

                                return Ok(result);
                            }
                            Err(err) => {
                                if !init_on_start {
                                    return Err(err);
                                }

                                println!("Can not load file from SSH. Error: {}. Retrying...", err);
                                tokio::time::sleep(Duration::from_secs(3)).await;
                            }
                        }
                    }
                }
            },
        }
    }
}

async fn loading_file_from_ssh(
    ssh_configuration: &SshConfiguration,
    path: &str,
) -> Result<Vec<u8>, String> {
    let ssh_session = get_ssh_session(ssh_configuration, path).await?;

    let result = ssh_session
        .download_remote_file(&path, Duration::from_secs(5))
        .await;

    if let Err(err) = result {
        match ssh_configuration.credentials.as_ref() {
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

    Ok(result)
}

async fn loading_content_from_http_via_ssh(
    ssh_configuration: &SshConfiguration,
    remote_host: &RemoteHost,
) -> Result<Vec<u8>, String> {
    use crate::http_client::SshConnector;
    use my_ssh::*;

    let connector = SshConnector {
        use_connection_pool: false,
        ssh_credentials: ssh_configuration.credentials.clone(),
        remote_host: remote_host.clone(),
        debug: false,
    };

    let http_client: MyHttpClient<SshAsyncChannel, SshConnector> = MyHttpClient::new(connector);

    let http_request = MyHttpRequestBuilder::new(Method::GET, remote_host.as_str()).build();

    let response = http_client
        .do_request(&http_request, Duration::from_secs(5))
        .await
        .map_err(|err| format!("{:?}", err))?;

    let response = response.into_response();

    let body = response.into_body();

    let body = body.collect().await.map_err(|err| format!("{:?}", err))?;

    let body = body.to_bytes();

    Ok(body.into())
}

async fn get_ssh_session(
    ssh_configuration: &SshConfiguration,
    remote_resource: &str,
) -> Result<SshSession, String> {
    println!(
        "Loading file from remove resource using SSH. {}->{}",
        ssh_configuration.credentials.to_string(),
        remote_resource
    );

    let ssh_credentials = ssh_configuration.credentials.clone();

    let ssh_credentials = if let SshCredentials::PrivateKey {
        ssh_remote_host,
        ssh_remote_port,
        ssh_user_name,
        private_key,
        passphrase,
    } = ssh_credentials.as_ref()
    {
        let ssh_pass_phrase_id =
            format!("{}@{}:{}", ssh_user_name, ssh_remote_host, ssh_remote_port);

        let passphrase = match passphrase {
            Some(pass) => {
                println!(
                    "Passphrase is provided for SSH key for endpoint: {}",
                    ssh_pass_phrase_id
                );
                Some(pass.to_string())
            }
            None => {
                println!(
                    "Passphrase IS NOT provided for SSH key for endpoint: {}",
                    ssh_pass_phrase_id
                );
                let passkey = crate::app::CERT_PASS_KEYS.get(&ssh_pass_phrase_id).await;

                println!(
                    "There is a passkey for endpoint: '{}'. Result: {}",
                    ssh_pass_phrase_id,
                    passkey.is_some()
                );

                passkey
            }
        };

        let result = SshCredentials::PrivateKey {
            ssh_remote_host: ssh_remote_host.to_string(),
            ssh_remote_port: *ssh_remote_port,
            ssh_user_name: ssh_user_name.to_string(),
            private_key: private_key.to_string(),
            passphrase,
        };
        Arc::new(result)
    } else {
        ssh_credentials
    };

    Ok(SshSession::new(ssh_credentials))
}
