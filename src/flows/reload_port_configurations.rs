use std::sync::Arc;

use crate::app::AppContext;

pub async fn reload_port_configurations(
    app: &Arc<AppContext>,
    port_to_refresh: u16,
) -> Result<String, String> {
    let settings_model = crate::scripts::load_settings().await?;

    let mut updated_endpoints = 0;
    for (host_id, host_settings) in &settings_model.hosts {
        let endpoint_host = settings_model.get_endpoint_host_string(host_id)?;

        if endpoint_host.get_port() != port_to_refresh {
            continue;
        }

        let endpoint = endpoint_host.as_str().to_string();

        crate::scripts::update_host_configuration(
            app,
            &settings_model,
            endpoint_host,
            host_settings,
        )
        .await?;
        updated_endpoints += 1;
        println!("Configuration for host {} has been reloaded", endpoint);
    }

    if updated_endpoints == 0 {
        app.current_configuration
            .write(move |config| {
                config.listen_endpoints.remove(&port_to_refresh);
            })
            .await;
    }

    crate::scripts::sync_tcp_endpoints(app).await;

    Ok(format!("Updated {} endpoints", updated_endpoints))
}
