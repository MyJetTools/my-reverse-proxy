pub async fn refresh_ip_list_from_settings(ip_list: &str) -> Result<(), String> {
    let settings_model = crate::scripts::load_settings().await?;
    crate::scripts::refresh_ip_list_from_settings(&settings_model, ip_list).await?;
    Ok(())
}
