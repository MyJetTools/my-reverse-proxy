use encryption::aes::AesKey;
use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GatewayClientSettings {
    pub remote_host: String,
    pub encryption_key: String,

    pub debug: Option<bool>,
}

impl GatewayClientSettings {
    pub fn is_debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }

    pub fn get_encryption_key(&self) -> Result<AesKey, String> {
        if self.encryption_key.len() < 16 {
            return Err(
                "Encryption key for ClientGateway must have at least 16 symbols".to_string(),
            );
        }

        let mut result = self.encryption_key.as_bytes().to_vec();

        while result.len() < 48 {
            result.extend_from_slice(self.encryption_key.as_bytes());
        }

        if result.len() > 48 {
            result.truncate(48);
        }

        Ok(AesKey::new(result.as_slice()))
    }
}
