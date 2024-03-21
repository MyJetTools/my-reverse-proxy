use crate::{app::AppContext, http_proxy_pass::*};

pub async fn get_locations<'s>(
    app: &AppContext,
    host: &HostPort<'s>,
) -> Result<Vec<ProxyPassLocation>, ProxyPassError> {
    let result = app
        .settings_reader
        .get_hosts_configurations(app, host)
        .await?;

    if result.len() == 0 {
        return Err(ProxyPassError::NoConfigurationFound);
    }

    Ok(result)
    /*
    match proxy_pass_settings {
        crate::settings::HttpProxyPassRemoteEndpoint::Http(uri) => {
            configurations.push(
                ProxyPassConfiguration::new(
                    location,
                    HttpProxyPassRemoteEndpoint::Http(uri),
                    proxy_pass_settings.id,
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

     */
}
