use crate::{configurations::EndpointPort, settings_compiled::SettingsCompiled};

pub async fn reload_tcp_port_configurations(port_to_refresh: u16) -> Result<String, String> {
    let settings_model = SettingsCompiled::load_settings().await?;

    let mut updated_endpoints = 0;
    for (host_id, host_settings) in &settings_model.hosts {
        let endpoint_host = settings_model.get_endpoint_host_string(host_id)?;

        let EndpointPort::Tcp(port) = endpoint_host.get_port() else {
            continue;
        };

        if port != port_to_refresh {
            continue;
        }

        let endpoint = endpoint_host.as_str().to_string();

        crate::scripts::update_host_configuration(&settings_model, endpoint_host, host_settings)
            .await?;
        updated_endpoints += 1;
        println!("Configuration for host {} has been reloaded", endpoint);
    }

    if updated_endpoints == 0 {
        crate::app::APP_CTX
            .current_configuration
            .write(move |config| {
                config.listen_tcp_endpoints.remove(&port_to_refresh);
            })
            .await;
    }

    crate::scripts::sync_endpoints().await;

    Ok(format!("Updated {} endpoints", updated_endpoints))
}

pub async fn reload_unix_configurations(host_to_refresh: &String) -> Result<String, String> {
    let settings_model = SettingsCompiled::load_settings().await?;

    let mut updated_endpoints = 0;
    for (host_id, host_settings) in &settings_model.hosts {
        let endpoint_host = settings_model.get_endpoint_host_string(host_id)?;

        if !endpoint_host.is_unix_socket() {
            continue;
        }

        if endpoint_host.as_str() != host_to_refresh.as_str() {
            continue;
        }

        let endpoint = endpoint_host.as_str().to_string();

        crate::scripts::update_host_configuration(&settings_model, endpoint_host, host_settings)
            .await?;
        updated_endpoints += 1;
        println!("Configuration for host {} has been reloaded", endpoint);
    }

    if updated_endpoints == 0 {
        crate::app::APP_CTX
            .current_configuration
            .write(move |config| {
                config.listen_unix_socket_endpoints.remove(host_to_refresh);
            })
            .await;
    }
    crate::scripts::sync_endpoints().await;

    Ok(format!("Updated {} endpoints", updated_endpoints))
}
