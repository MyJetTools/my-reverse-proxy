use crate::settings::SettingsModel;

pub async fn load_settings() -> Result<SettingsModel, String> {
    crate::settings::SettingsModel::load(".my-reverse-proxy").await
}
