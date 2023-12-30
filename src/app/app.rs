use crate::settings::SettingsReader;

pub struct AppContext {
    pub settings_reader: SettingsReader,
}

impl AppContext {
    pub fn new(settings_reader: SettingsReader) -> Self {
        Self { settings_reader }
    }
}
