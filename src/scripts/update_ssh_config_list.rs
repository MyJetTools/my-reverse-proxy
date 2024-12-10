use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{app::AppContext, configurations::SshConfig, settings::*};

pub async fn update_ssh_config_list(app: &Arc<AppContext>, settings_model: &SettingsModel) {
    let mut to_add = Vec::new();

    match settings_model.ssh.as_ref() {
        Some(ssh_config) => {
            for (config_id, settings) in ssh_config {
                if let Some(password) = &settings.password {
                    let password = match super::apply_variables(settings_model, password) {
                        Ok(value) => value,
                        Err(err) => {
                            println!("Skipping ssh configuration [{}]. Error: {}", config_id, err);
                            continue;
                        }
                    };

                    to_add.push((
                        config_id.to_string(),
                        SshConfig::Password(password.to_string()),
                    ));
                } else if let Some(private_key_file) = &settings.private_key_file {
                    let private_key_file =
                        match super::apply_variables(settings_model, private_key_file) {
                            Ok(private_key_file) => private_key_file,
                            Err(err) => {
                                println!(
                                    "Skipping loading ssh private key file {}. Error: {}",
                                    private_key_file.as_str(),
                                    err
                                );
                                continue;
                            }
                        };

                    let file_source = match OverSshConnectionSettings::try_parse(
                        private_key_file.as_str(),
                    ) {
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

                    match super::load_file(app, &file_source).await {
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
        }
        None => {}
    }

    app.ssh_config_list.clear_and_init(to_add.into_iter()).await;
}
