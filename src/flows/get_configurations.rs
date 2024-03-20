use crate::{
    app::AppContext,
    http_server::{HostPort, ProxyPassConfiguration, ProxyPassError},
    settings::HttpProxyPassRemoteEndpoint,
};

pub async fn get_configurations<'s>(
    app: &AppContext,
    host: &HostPort<'s>,
) -> Result<Vec<ProxyPassConfiguration>, ProxyPassError> {
    let mut configurations: Vec<ProxyPassConfiguration> = Vec::new();

    let locations = app.settings_reader.get_configurations(host).await;

    if locations.len() == 0 {
        return Err(ProxyPassError::NoConfigurationFound);
    }

    let mut id = app.get_id();
    println!("[{}] -------- Connected", id);
    for (location, proxy_pass_settings) in locations {
        match proxy_pass_settings {
            crate::settings::HttpProxyPassRemoteEndpoint::Http(uri) => {
                configurations.push(
                    ProxyPassConfiguration::new(
                        location,
                        HttpProxyPassRemoteEndpoint::Http(uri),
                        id,
                    )
                    .into(),
                );
            }
            crate::settings::HttpProxyPassRemoteEndpoint::Http2(uri) => {
                configurations.push(
                    ProxyPassConfiguration::new(
                        location,
                        HttpProxyPassRemoteEndpoint::Http2(uri),
                        id,
                    )
                    .into(),
                );
            }
            crate::settings::HttpProxyPassRemoteEndpoint::Http1OverSsh(ssh_config) => {
                configurations.push(
                    ProxyPassConfiguration::new(
                        location,
                        HttpProxyPassRemoteEndpoint::Http1OverSsh(ssh_config),
                        id,
                    )
                    .into(),
                );
            }
            crate::settings::HttpProxyPassRemoteEndpoint::Http2OverSsh(ssh_config) => {
                configurations.push(
                    ProxyPassConfiguration::new(
                        location,
                        HttpProxyPassRemoteEndpoint::Http2OverSsh(ssh_config),
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
