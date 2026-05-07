use crate::{settings_compiled::SettingsCompiled, types::WhiteListedIpList};

pub async fn refresh_ip_list_from_settings(
    settings_model: &SettingsCompiled,
    white_list_ip_id: &str,
) -> Result<(), String> {
    let ip_list = match settings_model.ip_white_lists.get(white_list_ip_id) {
        Some(ip_list) => ip_list,
        None => {
            return Err(format!("Ip list with id '{}' not found", white_list_ip_id,));
        }
    };

    let white_list_ip_list = WhiteListedIpList::new(ip_list);

    crate::app::APP_CTX
        .current_configuration
        .write(|config| {
            config
                .white_list_ip_list
                .insert_or_update(white_list_ip_id.to_string(), white_list_ip_list)
        })
        .await;

    Ok(())
}
