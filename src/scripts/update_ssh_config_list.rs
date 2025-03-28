use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{configurations::*, settings_compiled::SettingsCompiled};

pub async fn update_ssh_config_list(settings_model: &SettingsCompiled) {
    let mut to_add = Vec::new();

    for (config_id, settings) in &settings_model.ssh {
        if let Some(password) = &settings.password {
            to_add.push((
                config_id.to_string(),
                SshConfig::Password(password.to_string()),
            ));
        } else if let Some(private_key_file) = &settings.private_key_file {
            let file_source = match OverSshConnectionSettings::try_parse(private_key_file.as_str())
            {
                Some(file_source) => file_source,
                None => {
                    println!(
                        "Skipping ssh configuration {}. Invalid SSL private Key file source {}",
                        config_id,
                        private_key_file.as_str(),
                    );
                    continue;
                }
            };

            match super::load_file(&file_source, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT).await
            {
                Ok(private_key) => {
                    let private_key = match String::from_utf8(private_key) {
                        Ok(private_key) => private_key,
                        Err(err) => {
                            println!(
                                        "Skipping ssh configuration {}. Can not load private key from [{}]. Err: {}",
                                        config_id,
                                        private_key_file.as_str(),
                                        err
                                    );
                            continue;
                        }
                    };

                    to_add.push((
                        config_id.to_string(),
                        SshConfig::PrivateKey {
                            private_key,
                            pass_phrase: settings.passphrase.clone(),
                        },
                    ));
                }
                Err(err) => {
                    println!(
                        "Skipping loading ssh private key file {}. Error: {}",
                        private_key_file.as_str(),
                        err
                    )
                }
            }
        }
    }

    crate::app::APP_CTX
        .ssh_config_list
        .clear_and_init(to_add.into_iter())
        .await;
}
