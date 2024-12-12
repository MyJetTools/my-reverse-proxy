use std::sync::Arc;

use crate::{app::AppContext, settings::*};

pub async fn get_endpoint_users_list(
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

    super::refresh_users_list_from_settings(app, settings_model, allowed_users_list_id).await?;

    Ok(Some(allowed_users_list_id.to_string()))
}
