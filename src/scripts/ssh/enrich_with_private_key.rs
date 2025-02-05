use std::sync::Arc;

use my_ssh::{ssh_settings::OverSshConnectionSettings, SshCredentials};

use crate::settings::{SettingsModel, SshConfigSettings};

pub async fn enrich_with_private_key_or_password(
    over_ssh_connection: OverSshConnectionSettings,
    settings_model: &SettingsModel,
) -> Result<OverSshConnectionSettings, String> {
    let ssh_credentials = match over_ssh_connection.ssh_credentials {
        Some(ssh_credentials) => ssh_credentials,
        None => return Ok(over_ssh_connection),
    };

    if let Some(ssh_config) = &settings_model.ssh {
        let ssh_credentials_as_string = ssh_credentials.to_string();

        if let Some(config) = ssh_config.get(ssh_credentials_as_string.as_str()) {
            return apply_ssh_config(
                ssh_credentials,
                config,
                over_ssh_connection.remote_resource_string,
            )
            .await;
        }
    }

    Ok(OverSshConnectionSettings {
        ssh_credentials: Some(ssh_credentials),
        remote_resource_string: over_ssh_connection.remote_resource_string,
    })
}

async fn apply_ssh_config(
    ssh_credentials: Arc<SshCredentials>,
    ssh_config_settings: &SshConfigSettings,
    remote_resource_string: String,
) -> Result<OverSshConnectionSettings, String> {
    if let Some(password) = ssh_config_settings.password.as_ref() {
        let (host, port) = ssh_credentials.get_host_port();
        return Ok(OverSshConnectionSettings {
            ssh_credentials: Some(
                SshCredentials::UserNameAndPassword {
                    ssh_remote_host: host.to_string(),
                    ssh_remote_port: port,
                    ssh_user_name: ssh_credentials.get_user_name().to_string(),
                    password: password.to_string(),
                }
                .into(),
            ),
            remote_resource_string,
        });
    }

    if let Some(private_key_file) = ssh_config_settings.private_key_file.as_ref() {
        let private_key_file = rust_extensions::file_utils::format_path(private_key_file);
        let load_result = tokio::fs::read_to_string(private_key_file.as_str()).await;

        if let Err(err) = load_result {
            return Err(format!(
                "Can not load private key from [{}]. Err: {}",
                private_key_file.as_str(),
                err
            ));
        }

        let (host, port) = ssh_credentials.get_host_port();

        let file_content = load_result.unwrap();

        return Ok(OverSshConnectionSettings {
            ssh_credentials: Some(
                SshCredentials::PrivateKey {
                    ssh_remote_host: host.to_string(),
                    ssh_remote_port: port,
                    ssh_user_name: ssh_credentials.get_user_name().to_string(),
                    private_key: file_content,
                    passphrase: ssh_config_settings.passphrase.clone(),
                }
                .into(),
            ),
            remote_resource_string,
        });
    }

    Ok(OverSshConnectionSettings {
        ssh_credentials: Some(ssh_credentials),
        remote_resource_string,
    })
}
