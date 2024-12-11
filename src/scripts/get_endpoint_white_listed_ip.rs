use std::sync::Arc;

use crate::{app::AppContext, settings::*, types::WhiteListedIpList};

pub async fn get_endpoint_white_listed_ip(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    host_settings: &HostSettings,
) -> Result<Option<String>, String> {
    let white_list_ip_id = super::get_from_host_or_templates(
        settings_model,
        host_settings,
        |host_settings| host_settings.endpoint.whitelisted_ip.as_ref(),
        |templates| templates.whitelisted_ip.as_ref(),
    )?;

    if white_list_ip_id.is_none() {
        return Ok(None);
    }

    let white_list_ip_id = white_list_ip_id.unwrap();

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

    Ok(Some(white_list_ip_id.to_string()))
}
