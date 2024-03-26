use crate::{app::AppContext, http_proxy_pass::*};

pub async fn get_locations<'s>(
    app: &AppContext,
    host: &HostPort<'s>,
) -> Result<Vec<ProxyPassLocation>, ProxyPassError> {
    let result = app.settings_reader.get_locations(app, host).await?;

    if result.len() == 0 {
        return Err(ProxyPassError::NoConfigurationFound);
    }

    for location in &result {
        println!(
            "Request  {:?}:{} got locations: {}->{}",
            host.get_host(),
            host.get_port(),
            location.path,
            location.content_source.to_string()
        );
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
