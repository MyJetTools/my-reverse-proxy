use serde::*;

use super::LocalFilePath;

pub enum SshConfigOption {
    AsPassword(String),
    AsPrivateKeyFile {
        file_path: LocalFilePath,
        passphrase: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SshConfigSettings {
    pub password: Option<String>,
    pub private_key_file: Option<String>,
    pub passphrase: Option<String>,
}

impl SshConfigSettings {
    pub fn get_option(&self) -> Result<SshConfigOption, String> {
        if let Some(password) = self.password.as_ref() {
            return Ok(SshConfigOption::AsPassword(password.to_string()));
        }

        if self.private_key_file.is_none() {
            return Err("Either password or private_key_file must be set".to_string());
        }

        let private_key_file = self.private_key_file.as_ref().unwrap();

        Ok(SshConfigOption::AsPrivateKeyFile {
            file_path: LocalFilePath::new(private_key_file.to_string()),
            passphrase: self.passphrase.clone(),
        })
    }
}
