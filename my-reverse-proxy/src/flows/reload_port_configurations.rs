use crate::settings_compiled::SettingsCompiled;

pub async fn reload_tcp_port_configurations(port_to_refresh: u16) -> Result<String, String> {
    let settings_model = SettingsCompiled::load_settings().await?;

    let mut new_config_for_port: Option<crate::configurations::ListenConfiguration> = None;

    for (host_id, host_settings) in &settings_model.hosts {
        let endpoint_host = settings_model.get_endpoint_host_string(host_id)?;

        let crate::configurations::EndpointPort::Tcp(port) = endpoint_host.get_port() else {
            continue;
        };

        if port != port_to_refresh {
            continue;
        }

        let compiled = crate::scripts::compile_host_configuration(
            &settings_model,
            endpoint_host.clone(),
            host_settings,
            new_config_for_port.clone(),
        )
        .await?;
        new_config_for_port = Some(compiled);
    }

    let had_config = new_config_for_port.is_some();

    crate::app::APP_CTX
        .current_configuration
        .write(move |config| {
            config.error_configurations.remove(&port_to_refresh.to_string());

            match new_config_for_port {
                Some(cfg) => {
                    config.listen_tcp_endpoints.insert(port_to_refresh, cfg);
                }
                None => {
                    config.listen_tcp_endpoints.remove(&port_to_refresh);
                }
            }
        })
        .await;

    crate::scripts::sync_endpoints().await;

    if had_config {
        Ok(format!("Updated TCP port {}", port_to_refresh))
    } else {
        Ok(format!("Removed TCP port {}", port_to_refresh))
    }
}

pub async fn reload_unix_configurations(host_to_refresh: &String) -> Result<String, String> {
    let settings_model = SettingsCompiled::load_settings().await?;

    let mut new_config_for_host: Option<crate::configurations::ListenConfiguration> = None;

    for (host_id, host_settings) in &settings_model.hosts {
        let endpoint_host = settings_model.get_endpoint_host_string(host_id)?;

        if !endpoint_host.is_unix_socket() {
            continue;
        }

        if endpoint_host.as_str() != host_to_refresh.as_str() {
            continue;
        }

        let compiled = crate::scripts::compile_host_configuration(
            &settings_model,
            endpoint_host.clone(),
            host_settings,
            new_config_for_host.clone(),
        )
        .await?;
        new_config_for_host = Some(compiled);
    }

    let had_config = new_config_for_host.is_some();
    let unix_path = std::sync::Arc::new(host_to_refresh.clone());
    let host_to_refresh_clone = host_to_refresh.clone();

    crate::app::APP_CTX
        .current_configuration
        .write(move |config| {
            config.error_configurations.remove(host_to_refresh_clone.as_str());

            match new_config_for_host {
                Some(cfg) => {
                    config.listen_unix_socket_endpoints.insert(unix_path.clone(), cfg);
                }
                None => {
                    config.listen_unix_socket_endpoints.remove(&unix_path);
                }
            }
        })
        .await;

    crate::scripts::sync_endpoints().await;

    if had_config {
        Ok(format!("Updated Unix socket {}", host_to_refresh))
    } else {
        Ok(format!("Removed Unix socket {}", host_to_refresh))
    }
}
