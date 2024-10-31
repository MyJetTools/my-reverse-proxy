use std::sync::{
    atomic::{AtomicI64, AtomicIsize, Ordering},
    Arc,
};

use encryption::aes::AesKey;
use rust_extensions::AppStates;
use tokio::sync::RwLock;

use crate::{
    configurations::*,
    settings::{ConnectionsSettingsModel, SettingsModel},
};

use super::LocalPortAllocator;

pub const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
pub const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub struct AppContext {
    pub http_connections: AtomicIsize,
    id: AtomicI64,
    pub connection_settings: ConnectionsSettingsModel,
    //pub saved_client_certs: SavedClientCert,
    pub token_secret_key: AesKey,
    current_app_configuration: RwLock<Option<Arc<AppConfiguration>>>,
    pub states: Arc<AppStates>,
    pub local_port_allocator: LocalPortAllocator,
}

impl AppContext {
    pub fn new(settings_model: SettingsModel) -> Self {
        let connection_settings = settings_model.get_connections_settings();

        let token_secret_key = if let Some(session_key) = settings_model.get_session_key() {
            AesKey::new(get_token_secret_key_from_settings(session_key.as_bytes()).as_slice())
        } else {
            AesKey::new(generate_random_token_secret_key().as_slice())
        };

        Self {
            http_connections: AtomicIsize::new(0),
            id: AtomicI64::new(0),
            connection_settings,
            // saved_client_certs: SavedClientCert::new(),
            token_secret_key,
            current_app_configuration: RwLock::new(None),
            states: Arc::new(AppStates::create_initialized()),
            local_port_allocator: LocalPortAllocator::new(),
        }
    }

    pub async fn set_current_app_configuration(&self, app_config: AppConfiguration) {
        let mut current_app_configuration = self.current_app_configuration.write().await;
        *current_app_configuration = Some(Arc::new(app_config));
    }

    pub async fn get_current_app_configuration(&self) -> Arc<AppConfiguration> {
        self.current_app_configuration
            .read()
            .await
            .as_ref()
            .unwrap()
            .clone()
    }

    pub async fn try_get_current_app_configuration(&self) -> Option<Arc<AppConfiguration>> {
        let result = self.current_app_configuration.read().await;
        result.clone()
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
