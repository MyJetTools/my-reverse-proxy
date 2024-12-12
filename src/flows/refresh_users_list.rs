use std::sync::Arc;

use crate::app::AppContext;

pub async fn refresh_users_list(app: &Arc<AppContext>, users_list_id: &str) -> Result<(), String> {
    let settings_model = crate::scripts::load_settings().await?;
    crate::scripts::refresh_users_list(app, &settings_model, users_list_id).await?;
    Ok(())
}
