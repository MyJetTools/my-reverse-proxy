use std::sync::Arc;

use crate::{app::AppContext, configurations::ListenConfiguration, settings::*};

pub async fn delete_http_endpoint_if_exists(
    app: &Arc<AppContext>,
    settings: &SettingsModel,
    endpoint_id: &str,
) -> Result<(), String> {
    let host_endpoint = settings.get_endpoint_host_string(endpoint_id)?;
    let port = host_endpoint.get_port();

    let listen_configuration = app
        .current_configuration
        .get(move |config| config.listen_endpoints.get(&port).cloned())
        .await;

    if listen_configuration.is_none() {
        return Ok(());
    }

    let listen_configuration = listen_configuration.unwrap();

    if let ListenConfiguration::Http(http_config) = listen_configuration {
        if let Some(new_configuration) = http_config.delete_configuration(&host_endpoint) {
            if new_configuration.endpoints.is_empty() {
                app.current_configuration
                    .write(move |config| {
                        config.listen_endpoints.remove(&port);
                    })
                    .await;
            } else {
                app.current_configuration
                    .write(move |config| {
                        config
                            .listen_endpoints
                            .insert(port, ListenConfiguration::Http(new_configuration.into()));
                    })
                    .await;
            }
        }
    }

    Ok(())
}
