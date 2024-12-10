use crate::{configurations::EndpointHttpHostString, settings::*};

pub fn get_endpoint_user_list(
    settings_model: &SettingsModel,
    host_endpoint: &EndpointHttpHostString,
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
            "Endpoint {} has a user_list with id {} which is not found",
            host_endpoint.as_str(),
            allowed_users_list_id
        ));
    };

    let user_list = if let Some(user_list) = allowed_users_dict.get(allowed_users_list_id) {
        user_list
    } else {
        return Err(format!(
            "Endpoint {} has a user_list with id {} which is not found",
            host_endpoint.as_str(),
            allowed_users_list_id
        ));
    };

    let mut users = Vec::new();

    for user in user_list {
        users.push(super::apply_variables(settings_model, user)?.to_string());
    }

    Ok(Some(allowed_users_list_id.to_string()))
}
