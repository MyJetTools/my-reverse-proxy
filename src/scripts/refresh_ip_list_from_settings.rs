use std::sync::Arc;

use crate::{app::AppContext, settings::SettingsModel, types::WhiteListedIpList};

pub async fn refresh_ip_list_from_settings(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    white_list_ip_id: &str,
) -> Result<(), String> {
    let ip_white_lists = match settings_model.ip_white_lists.as_ref() {
        Some(ip_white_lists) => ip_white_lists,
        None => {
            return Err(format!("Ip list with id '{}' not found", white_list_ip_id));
        }
    };

    let ip_list = match ip_white_lists.get(white_list_ip_id) {
        Some(ip_list) => ip_list,
        None => {
            return Err(format!("Ip list with id '{}' not found", white_list_ip_id,));
        }
    };

    let white_list_ip_list = WhiteListedIpList::new(ip_list);

    app.current_configuration
        .write(|config| {
            config
                .white_list_ip_list
                .insert_or_update(white_list_ip_id.to_string(), white_list_ip_list)
        })
        .await;

    Ok(())
}
