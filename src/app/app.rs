use std::sync::atomic::AtomicIsize;

use crate::settings::SettingsReader;

pub struct AppContext {
    pub settings_reader: SettingsReader,
    pub http_connections: AtomicIsize,
}

impl AppContext {
    pub fn new(settings_reader: SettingsReader) -> Self {
        Self {
            settings_reader,
            http_connections: AtomicIsize::new(0),
        }
    }
}
