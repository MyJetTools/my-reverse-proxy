use std::sync::atomic::{AtomicI64, AtomicIsize, Ordering};

use crate::settings::{ConnectionsSettingsModel, SettingsReader};

use super::SavedClientCert;

pub struct AppContext {
    pub settings_reader: SettingsReader,
    pub http_connections: AtomicIsize,
    id: AtomicI64,
    pub connection_settings: ConnectionsSettingsModel,
    pub saved_client_certs: SavedClientCert,
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
        }
    }

    pub fn get_id(&self) -> i64 {
        self.id.fetch_add(1, Ordering::SeqCst)
    }
}
