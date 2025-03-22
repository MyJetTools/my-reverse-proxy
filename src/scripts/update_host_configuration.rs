use crate::{
    configurations::EndpointHttpHostString, settings::*, settings_compiled::SettingsCompiled,
};

pub async fn update_host_configuration(
    settings_model: &SettingsCompiled,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
) -> Result<(), String> {
    let port = host_endpoint.get_port();
    let configuration = crate::scripts::compile_host_configuration(
        &settings_model,
        host_endpoint.clone(),
        host_settings,
    )
    .await?;

    crate::app::APP_CTX
        .current_configuration
        .write(move |config| {
            config.error_configurations.remove(host_endpoint.as_str());

            config.listen_endpoints.insert(port, configuration);
        })
        .await;

    Ok(())
}
