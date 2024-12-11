use std::{collections::HashSet, sync::Arc};

use crate::{app::AppContext, settings::*};

pub async fn get_endpoint_user_list(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    host_settings: &HostSettings,
) -> Result<Option<String>, String> {
    let allowed_users_list_id =
        if let Some(allowed_users_id) = host_settings.endpoint.allowed_users.as_ref() {
            allowed_users_id
        } else {
            return Ok(None);
        };

    let allowed_users_dict = if let Some(allowed_users_dict) = settings_model.allowed_users.as_ref()
    {
        allowed_users_dict
    } else {
        return Err(format!(
            "User list with id '{}' is not found",
            allowed_users_list_id
        ));
    };

    let user_list = if let Some(user_list) = allowed_users_dict.get(allowed_users_list_id) {
        user_list
    } else {
        return Err(format!(
            "User list with id {} is not found",
            allowed_users_list_id
        ));
    };

    let mut users = HashSet::new();

    for user in user_list {
        users.insert(super::apply_variables(settings_model, user)?.to_string());
    }

    app.allowed_users_list
        .insert(allowed_users_list_id.to_string(), users)
        .await;

    Ok(Some(allowed_users_list_id.to_string()))
}
