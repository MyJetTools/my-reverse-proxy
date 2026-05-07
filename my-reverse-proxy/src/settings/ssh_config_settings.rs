use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SshConfigSettings {
    pub password: Option<String>,
    pub private_key_file: Option<String>,
    pub passphrase: Option<String>,
}
