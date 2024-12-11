use std::sync::Arc;

use crate::app::AppContext;

pub async fn load_everything_from_settings(app: &Arc<AppContext>) {
    let settings_model = crate::scripts::load_settings().await.unwrap();

    crate::scripts::update_ssh_config_list(app, &settings_model).await;

    for (host_id, host_settings) in &settings_model.hosts {
        let endpoint_host = match settings_model.get_endpoint_host_string(host_id) {
            Ok(host_id) => host_id,
            Err(err) => {
                println!(
                    "Error applying variables to host {}. Err is: {}",
                    host_id, err
                );
                continue;
            }
        };

        if let Err(err) = crate::scripts::update_host_configuration(
            app,
            &settings_model,
            endpoint_host.clone(),
            host_settings,
        )
        .await
        {
            app.current_configuration
                .add_http_configuration_error(&endpoint_host, err.clone())
                .await;

            println!(
                "Error loading host configuration {}. Err is: {}",
                endpoint_host.as_str(),
                err
            );
        }
    }

    crate::scripts::sync_tcp_endpoints(&app).await;
}
