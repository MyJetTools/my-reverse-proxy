use std::collections::HashSet;

use crate::settings_compiled::SettingsCompiled;

pub async fn refresh_users_list_from_settings(
    settings: &SettingsCompiled,
    users_list_id: &str,
) -> Result<(), String> {
    let user_list = if let Some(user_list) = settings.allowed_users.get(users_list_id) {
        user_list
    } else {
        return Err(format!("User list with id {} is not found", users_list_id));
    };

    let mut users = HashSet::new();

    for user in user_list {
        users.insert(user.to_string());
    }

    crate::app::APP_CTX
        .allowed_users_list
        .insert(users_list_id.to_string(), users)
        .await;

    Ok(())
}
