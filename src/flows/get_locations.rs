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
}
