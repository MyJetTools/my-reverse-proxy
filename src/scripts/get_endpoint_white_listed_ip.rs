use crate::settings::*;

pub async fn get_endpoint_white_listed_ip(
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

    super::refresh_ip_list_from_settings(settings_model, white_list_ip_id).await?;

    Ok(Some(white_list_ip_id.to_string()))
}
