use crate::settings_compiled::SettingsCompiled;

pub async fn load_everything_from_settings() {
    let settings_model = SettingsCompiled::load_settings().await.unwrap();

    crate::scripts::update_ssh_config_list(&settings_model).await;

    for (host_id, host_settings) in &settings_model.hosts {
        println!("HostId: {}", host_id);
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
            &settings_model,
            endpoint_host.clone(),
            host_settings,
        )
        .await
        {
            crate::app::APP_CTX
                .current_configuration
                .add_http_configuration_error(&endpoint_host, err.clone())
                .await;

            println!(
                "Error loading host configuration {}. Err is: {}",
                endpoint_host.as_str(),
                err
            );
        }
    }

    println!("Kicking off tcp endpoints");

    crate::scripts::sync_endpoints().await;
}
