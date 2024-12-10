use std::sync::Arc;

use crate::{app::AppContext, configurations::EndpointHttpHostString};

pub async fn reload_endpoint_configuration(
    app: &Arc<AppContext>,
    host_id_to_refresh: &str,
) -> Result<String, String> {
    let settings_model = crate::scripts::load_settings().await?;

    for (host_id, host_settings) in &settings_model.hosts {
        let host_id = crate::scripts::apply_variables(&settings_model, host_id)?;

        if rust_extensions::str_utils::compare_strings_case_insensitive(
            host_id.as_str(),
            host_id_to_refresh,
        ) {
            let host_endpoint = EndpointHttpHostString::new(host_id.to_string())?;

            crate::scripts::update_host_configuration(
                app,
                &settings_model,
                host_endpoint,
                host_settings,
            )
            .await?;

            crate::scripts::sync_tcp_endpoints(app).await;
            return Ok(format!(
                "Host configuration {} has been reloaded",
                host_id_to_refresh
            ));
        }
    }

    match crate::scripts::delete_http_endpoint_if_exists(app, &settings_model, host_id_to_refresh)
        .await
    {
        Ok(_) => {
            crate::scripts::sync_tcp_endpoints(app).await;
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
