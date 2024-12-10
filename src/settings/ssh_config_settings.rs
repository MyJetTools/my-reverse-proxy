use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SshConfigSettings {
    pub password: Option<String>,
    pub private_key_file: Option<String>,
    pub passphrase: Option<String>,
}

/*
pub enum SshConfigOption {
    AsPassword(String),
    AsPrivateKeyFile {
        file_path: LocalFilePath,
        passphrase: Option<String>,
    },
}
 */
