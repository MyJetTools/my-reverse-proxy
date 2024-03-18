use std::sync::atomic::{AtomicI64, AtomicIsize, Ordering};

use crate::settings::{ConnectionsSettingsModel, SettingsReader};

pub struct AppContext {
    pub settings_reader: SettingsReader,
    pub http_connections: AtomicIsize,
    id: AtomicI64,
    pub connection_settings: ConnectionsSettingsModel,
}

impl AppContext {
    pub fn new(
        settings_reader: SettingsReader,
        connection_settings: ConnectionsSettingsModel,
    ) -> Self {
        Self {
            settings_reader,
            http_connections: AtomicIsize::new(0),
            id: AtomicI64::new(0),
            connection_settings,
        }
    }

    pub fn get_id(&self) -> i64 {
        self.id.fetch_add(1, Ordering::SeqCst)
    }
}
