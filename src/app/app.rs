use std::sync::atomic::{AtomicI64, AtomicIsize, Ordering};

use crate::settings::SettingsReader;

pub struct AppContext {
    pub settings_reader: SettingsReader,
    pub http_connections: AtomicIsize,
    id: AtomicI64,
}

impl AppContext {
    pub fn new(settings_reader: SettingsReader) -> Self {
        Self {
            settings_reader,
            http_connections: AtomicIsize::new(0),
            id: AtomicI64::new(0),
        }
    }

    pub fn get_id(&self) -> i64 {
        self.id.fetch_add(1, Ordering::SeqCst)
    }
}
