use crate::settings::SettingsModel;

pub async fn load_settings() -> Result<SettingsModel, String> {
    crate::settings::SettingsModel::load_async().await
}
