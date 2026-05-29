use crate::{
    configurations::{EndpointHttpHostString, EndpointPort},
    settings::*,
    settings_compiled::SettingsCompiled,
};

/// Compiles a single host and writes it into the LIVE configuration in place. Used by the targeted
/// per-port reload paths (`reload_endpoint_configuration`, `reload_port_configurations`). The full
/// reload (`load_everything_from_settings`) does NOT go through here — it builds a fresh map and
/// swaps it atomically so that removed hosts disappear.
pub async fn update_host_configuration(
    settings_model: &SettingsCompiled,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
) -> Result<(), String> {
    let tcp_port = host_endpoint.get_port();

    // Existing port config from the live configuration, so multiple hosts on the same port merge.
    let existing = match &tcp_port {
        EndpointPort::Tcp(port) => {
            let port = *port;
            crate::app::APP_CTX
                .current_configuration
                .get(move |config| config.listen_tcp_endpoints.get(&port).cloned())
                .await
        }
        EndpointPort::UnixSocket(unix_host) => {
            let unix_host = unix_host.clone();
            crate::app::APP_CTX
                .current_configuration
                .get(move |config| config.listen_unix_socket_endpoints.get(&unix_host).cloned())
                .await
        }
    };

    let configuration = crate::scripts::compile_host_configuration(
        &settings_model,
        host_endpoint.clone(),
        host_settings,
        existing,
    )
    .await?;

    let host_endpoint_str = host_endpoint.as_str().to_string();

    match tcp_port {
        EndpointPort::Tcp(tcp_port) => {
            crate::app::APP_CTX
                .current_configuration
                .write(move |config| {
                    config
                        .error_configurations
                        .remove(host_endpoint_str.as_str());

                    config.listen_tcp_endpoints.insert(tcp_port, configuration);
                })
                .await;
        }
        EndpointPort::UnixSocket(unix_host) => {
            crate::app::APP_CTX
                .current_configuration
                .write(move |config| {
                    config.error_configurations.remove(unix_host.as_str());

                    config
                        .listen_unix_socket_endpoints
                        .insert(unix_host, configuration);
                })
                .await;
        }
    }

    Ok(())
}
