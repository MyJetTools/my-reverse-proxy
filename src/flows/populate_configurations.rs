use crate::{
    app::AppContext,
    http_server::{HostPort, ProxyPassConfiguration, ProxyPassError},
    settings::ProxyPassRemoteEndpoint,
};

pub async fn get_configurations<'s, T>(
    app: &AppContext,
    host: &HostPort<'s, T>,
) -> Result<Vec<ProxyPassConfiguration>, ProxyPassError> {
    let mut configurations = Vec::new();

    let locations = app.settings_reader.get_configurations(host).await;

    if locations.len() == 0 {
        return Err(ProxyPassError::NoConfigurationFound);
    }

    let mut id = app.get_id();
    println!("[{}] -------- Connected", id);
    for (location, proxy_pass_settings) in locations {
        match proxy_pass_settings {
            crate::settings::ProxyPassRemoteEndpoint::Http(uri) => {
                configurations.push(
                    ProxyPassConfiguration::new(location, ProxyPassRemoteEndpoint::Http(uri), id)
                        .into(),
                );
            }
            crate::settings::ProxyPassRemoteEndpoint::Http2(uri) => {
                configurations.push(
                    ProxyPassConfiguration::new(location, ProxyPassRemoteEndpoint::Http2(uri), id)
                        .into(),
                );
            }
            crate::settings::ProxyPassRemoteEndpoint::Http1OverSsh(ssh_config) => {
                configurations.push(
                    ProxyPassConfiguration::new(
                        location,
                        ProxyPassRemoteEndpoint::Http1OverSsh(ssh_config),
                        id,
                    )
                    .into(),
                );
            }
            crate::settings::ProxyPassRemoteEndpoint::Http2OverSsh(ssh_config) => {
                configurations.push(
                    ProxyPassConfiguration::new(
                        location,
                        ProxyPassRemoteEndpoint::Http2OverSsh(ssh_config),
                        id,
                    )
                    .into(),
                );
            }
        }

        id += 1;
    }

    Ok(configurations)
}
