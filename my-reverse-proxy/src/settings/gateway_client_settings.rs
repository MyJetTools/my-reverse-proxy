use std::time::Duration;

use ed25519_dalek::SigningKey;
use serde::*;
use ssh_key::{private::Ed25519Keypair, PrivateKey};

use super::SshConfigSettings;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GatewayClientSettings {
    pub remote_host: String,
    pub ssh_credentials: String,
    pub compress: Option<bool>,

    pub debug: Option<bool>,
    pub allow_incoming_forward_connections: Option<bool>,
    pub connect_timeout_seconds: Option<u64>,
    pub sync_ssl_certificates: Option<Vec<String>>,
}

impl GatewayClientSettings {
    pub fn is_debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }

    pub fn get_supported_compression(&self) -> bool {
        self.compress.unwrap_or(false)
    }

    pub fn get_allow_incoming_forward_connections(&self) -> bool {
        self.allow_incoming_forward_connections.unwrap_or(false)
    }

    pub fn get_connect_timeout(&self) -> Duration {
        Duration::from_secs(self.connect_timeout_seconds.unwrap_or(5))
    }

    pub fn get_sync_ssl_certificates(&self) -> Vec<String> {
        self.sync_ssl_certificates.clone().unwrap_or_default()
    }

    pub fn load_signing_key(
        &self,
        client_id: &str,
        ssh_registry: &std::collections::HashMap<String, SshConfigSettings>,
    ) -> Result<SigningKey, String> {
        let ssh_entry = ssh_registry.get(self.ssh_credentials.as_str()).ok_or_else(|| {
            format!(
                "Gateway client '{client_id}': ssh_credentials '{}' not found in 'ssh:' registry",
                self.ssh_credentials
            )
        })?;

        let key_path = ssh_entry.private_key_file.as_deref().ok_or_else(|| {
            format!(
                "Gateway client '{client_id}': ssh entry '{}' has no private_key_file — gateway requires a private key, password-only entries are not supported",
                self.ssh_credentials
            )
        })?;

        let resolved = rust_extensions::file_utils::format_path(key_path).to_string();

        let raw = std::fs::read(resolved.as_str()).map_err(|err| {
            format!(
                "Gateway client '{client_id}': cannot read private_key_file '{resolved}': {err}"
            )
        })?;

        let private = PrivateKey::from_openssh(&raw).map_err(|err| {
            format!(
                "Gateway client '{client_id}': cannot parse OpenSSH private key '{resolved}': {err}"
            )
        })?;

        let unlocked = if private.is_encrypted() {
            let phrase = ssh_entry.passphrase.as_deref().ok_or_else(|| {
                format!(
                    "Gateway client '{client_id}': private key '{resolved}' is encrypted but ssh entry has no passphrase"
                )
            })?;
            private.decrypt(phrase.as_bytes()).map_err(|err| {
                format!(
                    "Gateway client '{client_id}': failed to decrypt private key '{resolved}' with passphrase: {err}"
                )
            })?
        } else {
            private
        };

        let keypair: &Ed25519Keypair = unlocked.key_data().ed25519().ok_or_else(|| {
            format!(
                "Gateway client '{client_id}': private key '{resolved}' is not Ed25519"
            )
        })?;

        Ok(SigningKey::from_bytes(&keypair.private.to_bytes()))
    }
}
