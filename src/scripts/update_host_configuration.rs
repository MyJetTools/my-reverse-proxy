use crate::{
    configurations::EndpointHttpHostString, settings::*, settings_compiled::SettingsCompiled,
};

pub async fn update_host_configuration(
    settings_model: &SettingsCompiled,
    host_endpoint: EndpointHttpHostString,
    host_settings: &HostSettings,
) -> Result<(), String> {
    let configuration = crate::scripts::compile_host_configuration(
        &settings_model,
        host_endpoint.clone(),
        host_settings,
    )
    .await?;

    let host_endpoint_str = host_endpoint.as_str().to_string();
    let tcp_port = host_endpoint.get_port();

    match tcp_port {
        crate::configurations::EndpointPort::Tcp(tcp_port) => {
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
        crate::configurations::EndpointPort::UnixSocket(unix_host) => {
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
