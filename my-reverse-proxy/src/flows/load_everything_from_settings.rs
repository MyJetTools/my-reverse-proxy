use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use crate::{
    configurations::{EndpointPort, ListenConfiguration},
    settings_compiled::SettingsCompiled,
};

pub async fn load_everything_from_settings() -> Result<(), String> {
    let settings_model = SettingsCompiled::load_settings().await?;

    crate::scripts::update_ssh_config_list(&settings_model).await;

    // Build the full new configuration into local maps first, then swap it into the live
    // configuration in a single write. This way endpoints that disappeared from the settings are
    // simply absent from the new maps (instead of lingering, as an in-place merge would leave them),
    // and `sync_endpoints` below tears down their listeners. The live config keeps serving the old
    // configuration untouched until the atomic swap, so there is no window of missing routes.
    let mut listen_tcp_endpoints: HashMap<u16, ListenConfiguration> = HashMap::new();
    let mut listen_unix_socket_endpoints: HashMap<Arc<String>, ListenConfiguration> = HashMap::new();
    let mut error_configurations: BTreeMap<String, String> = BTreeMap::new();

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

        // What we've already accumulated for this port (multiple hosts can share one port).
        let existing = match endpoint_host.get_port() {
            EndpointPort::Tcp(port) => listen_tcp_endpoints.get(&port).cloned(),
            EndpointPort::UnixSocket(unix_host) => {
                listen_unix_socket_endpoints.get(&unix_host).cloned()
            }
        };

        match crate::scripts::compile_host_configuration(
            &settings_model,
            endpoint_host.clone(),
            host_settings,
            existing,
        )
        .await
        {
            Ok(configuration) => match endpoint_host.get_port() {
                EndpointPort::Tcp(port) => {
                    listen_tcp_endpoints.insert(port, configuration);
                }
                EndpointPort::UnixSocket(unix_host) => {
                    listen_unix_socket_endpoints.insert(unix_host, configuration);
                }
            },
            Err(err) => {
                println!(
                    "Error loading host configuration {}. Err is: {}",
                    endpoint_host.as_str(),
                    err
                );
                error_configurations.insert(endpoint_host.as_str().to_string(), err);
            }
        }
    }

    println!("Applying new configuration (atomic swap)");

    crate::app::APP_CTX
        .current_configuration
        .write(move |config| {
            config.listen_tcp_endpoints = listen_tcp_endpoints;
            config.listen_unix_socket_endpoints = listen_unix_socket_endpoints;
            config.error_configurations = error_configurations;
        })
        .await;

    println!("Kicking off tcp endpoints");

    crate::scripts::sync_endpoints().await;

    // Keep a copy of what we just applied so it can be read back (e.g. via MCP)
    // without recompiling the files — and compared against a fresh compile.
    crate::app::APP_CTX
        .applied_settings
        .store(Some(Arc::new(settings_model)));

    Ok(())
}
