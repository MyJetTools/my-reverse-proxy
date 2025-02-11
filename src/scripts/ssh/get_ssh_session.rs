use std::sync::Arc;

use my_ssh::{SshCredentials, SshSession};

pub async fn get_ssh_session(
    ssh_credentials: &Arc<SshCredentials>,
) -> Result<Arc<SshSession>, String> {
    let ssh_config = crate::app::APP_CTX
        .ssh_config_list
        .get(ssh_credentials)
        .await;

    let ssh_credentials = if let Some(ssh_config) = ssh_config {
        match ssh_config.as_ref() {
            crate::configurations::SshConfig::PrivateKey {
                private_key,
                pass_phrase,
            } => {
                let (host, port) = ssh_credentials.get_host_port();
                let passphrase = if pass_phrase.is_none() {
                    let ssh_pass_key = crate::app::APP_CTX
                        .ssh_cert_pass_keys
                        .get(ssh_credentials.as_ref())
                        .await;
                    ssh_pass_key
                } else {
                    pass_phrase.clone()
                };
                let ssh_credentials = SshCredentials::PrivateKey {
                    ssh_remote_host: host.to_string(),
                    ssh_remote_port: port,
                    ssh_user_name: ssh_credentials.get_user_name().to_string(),
                    private_key: private_key.to_string(),
                    passphrase,
                };

                Arc::new(ssh_credentials)
            }
            crate::configurations::SshConfig::Password(password) => {
                let (host, port) = ssh_credentials.get_host_port();
                let ssh_credentials = SshCredentials::UserNameAndPassword {
                    ssh_remote_host: host.to_string(),
                    ssh_remote_port: port,
                    ssh_user_name: ssh_credentials.get_user_name().to_string(),
                    password: password.to_string(),
                };

                Arc::new(ssh_credentials)
            }
        }
    } else {
        ssh_credentials.clone()
    };

    /*
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
     */

    let session = my_ssh::SSH_SESSIONS_POOL
        .get_or_create(&ssh_credentials)
        .await;

    Ok(session)
}
