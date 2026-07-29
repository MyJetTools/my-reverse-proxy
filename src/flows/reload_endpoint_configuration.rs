use crate::{configurations::EndpointHttpHostString, settings_compiled::SettingsCompiled};

pub async fn reload_endpoint_configuration(host_id_to_refresh: &str) -> Result<String, String> {
    let settings_model = SettingsCompiled::load_settings().await?;

    for (host_id, host_settings) in &settings_model.hosts {
        if rust_extensions::str_utils::compare_strings_case_insensitive(
            host_id.as_str(),
            host_id_to_refresh,
        ) {
            let host_endpoint = EndpointHttpHostString::new(host_id.to_string())?;

            crate::scripts::update_host_configuration(
                &settings_model,
                host_endpoint,
                host_settings,
            )
            .await?;

            crate::scripts::sync_endpoints().await;
            return Ok(format!(
                "Host configuration {} has been reloaded",
                host_id_to_refresh
            ));
        }
    }

    match crate::scripts::delete_http_endpoint_if_exists(&settings_model, host_id_to_refresh).await
    {
        Ok(_) => {
            crate::scripts::sync_endpoints().await;
            return Ok(format!(
                "Host configuration {} has been reloaded",
                host_id_to_refresh
            ));
        }
        Err(_) => {
            return Err(format!(
                "Host configuration {} not found",
                host_id_to_refresh
            ));
        }
    }
}
