use std::sync::Arc;

use crate::app::AppContext;

pub async fn refresh_ip_list_from_settings(
    app: &Arc<AppContext>,
    ip_list: &str,
) -> Result<(), String> {
    let settings_model = crate::scripts::load_settings().await?;
    crate::scripts::refresh_ip_list_from_settings(app, &settings_model, ip_list).await?;
    Ok(())
}
