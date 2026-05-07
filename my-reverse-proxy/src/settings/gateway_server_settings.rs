use std::collections::HashSet;

use ed25519_dalek::VerifyingKey;
use serde::*;
use ssh_key::PublicKey;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GatewayServerSettings {
    pub port: u16,
    pub authorized_keys: Vec<String>,
    pub allowed_ip: Option<Vec<String>>,
    pub debug: Option<bool>,
}

impl GatewayServerSettings {
    pub fn is_debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }

    pub fn load_authorized_keys(&self) -> Result<Vec<VerifyingKey>, String> {
        if self.authorized_keys.is_empty() {
            return Err(
                "Gateway server: 'authorized_keys' must contain at least one path".to_string(),
            );
        }

        let mut keys = Vec::with_capacity(self.authorized_keys.len());
        for path in &self.authorized_keys {
            let resolved = rust_extensions::file_utils::format_path(path).to_string();
            let content = std::fs::read_to_string(resolved.as_str()).map_err(|err| {
                format!("Gateway server: cannot read pubkey file '{resolved}': {err}")
            })?;

            let pub_key = PublicKey::from_openssh(content.trim()).map_err(|err| {
                format!("Gateway server: cannot parse pubkey '{resolved}': {err}")
            })?;

            let ed25519 = pub_key.key_data().ed25519().ok_or_else(|| {
                format!("Gateway server: pubkey '{resolved}' is not Ed25519")
            })?;

            let verifying = VerifyingKey::from_bytes(&ed25519.0).map_err(|err| {
                format!("Gateway server: invalid Ed25519 pubkey in '{resolved}': {err}")
            })?;

            keys.push(verifying);
        }

        Ok(keys)
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
