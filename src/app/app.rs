use std::sync::{
    atomic::{AtomicI64, AtomicIsize, Ordering},
    Arc,
};

use encryption::aes::AesKey;
use rust_extensions::AppStates;
use tokio::sync::RwLock;

use crate::{
    app_configuration::AppConfiguration,
    settings::{ConnectionsSettingsModel, SettingsReader},
};

pub const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub struct AppContext {
    pub settings_reader: SettingsReader,
    pub http_connections: AtomicIsize,
    id: AtomicI64,
    pub connection_settings: ConnectionsSettingsModel,
    //pub saved_client_certs: SavedClientCert,
    pub token_secret_key: AesKey,
    pub current_app_configuration: RwLock<Option<AppConfiguration>>,
    pub states: Arc<AppStates>,
}

impl AppContext {
    pub async fn new(settings_reader: SettingsReader) -> Self {
        let connection_settings = settings_reader.get_connections_settings().await;

        let token_secret_key = if let Some(session_key) = settings_reader.get_session_key().await {
            AesKey::new(get_token_secret_key_from_settings(session_key.as_bytes()).as_slice())
        } else {
            AesKey::new(generate_random_token_secret_key().as_slice())
        };

        Self {
            settings_reader,
            http_connections: AtomicIsize::new(0),
            id: AtomicI64::new(0),
            connection_settings,
            // saved_client_certs: SavedClientCert::new(),
            token_secret_key,
            current_app_configuration: RwLock::new(None),
            states: Arc::new(AppStates::create_initialized()),
        }
    }

    pub fn get_id(&self) -> i64 {
        self.id.fetch_add(1, Ordering::SeqCst)
    }
}

fn generate_random_token_secret_key() -> Vec<u8> {
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

fn get_token_secret_key_from_settings(session_key: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(48);

    let mut key = vec![];

    while result.len() < 48 {
        if key.len() == 0 {
            key = session_key.to_vec();
        }

        result.push(key.pop().unwrap());
    }

    result
}
