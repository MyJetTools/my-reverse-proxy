use crate::settings_compiled::SettingsCompiled;

pub async fn refresh_users_list(users_list_id: &str) -> Result<(), String> {
    let settings_model = SettingsCompiled::load_settings().await?;
    crate::scripts::refresh_users_list_from_settings(&settings_model, users_list_id).await?;
    Ok(())
}
