use std::sync::atomic::{AtomicI64, AtomicIsize, Ordering};

use encryption::aes::AesKey;

use crate::settings::{ConnectionsSettingsModel, SettingsReader};

use super::SavedClientCert;

pub struct AppContext {
    pub settings_reader: SettingsReader,
    pub http_connections: AtomicIsize,
    id: AtomicI64,
    pub connection_settings: ConnectionsSettingsModel,
    pub saved_client_certs: SavedClientCert,
    pub token_secret_key: AesKey,
}

impl AppContext {
    pub async fn new(settings_reader: SettingsReader) -> Self {
        let connection_settings = settings_reader.get_connections_settings().await;
        Self {
            settings_reader,
            http_connections: AtomicIsize::new(0),
            id: AtomicI64::new(0),
            connection_settings,
            saved_client_certs: SavedClientCert::new(),
            token_secret_key: AesKey::new(generate_token_secret_key().as_slice()),
        }
    }

    pub fn get_id(&self) -> i64 {
        self.id.fetch_add(1, Ordering::SeqCst)
    }
}

fn generate_token_secret_key() -> Vec<u8> {
    let mut result = Vec::with_capacity(48);

    let mut key = vec![];

    while result.len() < 48 {
        if key.len() == 0 {
            key = uuid::Uuid::new_v4().as_bytes().to_vec();
        }

        result.push(key.pop().unwrap());
    }

    result
}
