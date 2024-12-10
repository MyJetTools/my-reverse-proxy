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

        let host_str_id = endpoint_host.as_str().to_string();

        if let Err(err) = crate::scripts::update_host_configuration(
            app,
            &settings_model,
            endpoint_host,
            host_settings,
        )
        .await
        {
            println!(
                "Error updating host configuration {}. Err is: {}",
                host_str_id, err
            );
        }
    }

    crate::scripts::sync_tcp_endpoints(&app).await;
}
