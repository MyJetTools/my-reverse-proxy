use std::sync::Arc;

use crate::{app::AppContext, configurations::EndpointHttpHostString, settings::*};

pub async fn update_host_configuration(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
) -> Result<(), String> {
    let port = host_endpoint.get_port();
    let configuration = crate::scripts::compile_host_configuration(
        app,
        &settings_model,
        host_endpoint.clone(),
        host_settings,
    )
    .await?;

    app.current_configuration
        .write(move |config| {
            config.error_configurations.remove(host_endpoint.as_str());

            config.listen_endpoints.insert(port, configuration);
        })
        .await;

    Ok(())
}
