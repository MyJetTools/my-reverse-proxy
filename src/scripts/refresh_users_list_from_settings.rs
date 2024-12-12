use std::{collections::HashSet, sync::Arc};

use crate::{app::AppContext, settings::SettingsModel};

pub async fn refresh_users_list_from_settings(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    users_list_id: &str,
) -> Result<(), String> {
    let allowed_users_dict = if let Some(allowed_users_dict) = settings_model.allowed_users.as_ref()
    {
        allowed_users_dict
    } else {
        return Err(format!(
            "User list with id '{}' is not found",
            users_list_id
        ));
    };

    let user_list = if let Some(user_list) = allowed_users_dict.get(users_list_id) {
        user_list
    } else {
        return Err(format!("User list with id {} is not found", users_list_id));
    };

    let mut users = HashSet::new();

    for user in user_list {
        users.insert(super::apply_variables(settings_model, user)?.to_string());
    }

    app.allowed_users_list
        .insert(users_list_id.to_string(), users)
        .await;

    Ok(())
}
