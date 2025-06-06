use std::collections::HashSet;

use encryption::aes::AesKey;
use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GatewayServerSettings {
    pub port: u16,
    pub encryption_key: String,
    pub allowed_ip: Option<Vec<String>>,
    pub debug: Option<bool>,
}

impl GatewayServerSettings {
    pub fn is_debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }

    pub fn get_encryption_key(&self) -> Result<AesKey, String> {
        if self.encryption_key.len() < 16 {
            return Err(
                "Encryption key for ServerGateway must have at least 16 symbols".to_string(),
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

    pub fn get_allowed_ip_list(&self) -> Option<HashSet<String>> {
        let items = self.allowed_ip.as_ref()?;

        let mut result = HashSet::new();

        for itm in items {
            result.insert(itm.to_string());
        }

        Some(result)
    }
}
