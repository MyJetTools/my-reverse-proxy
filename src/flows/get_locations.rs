use crate::{app::AppContext, http_proxy_pass::*};

pub async fn get_locations<'s>(
    app: &AppContext,
    endpoint_info: &ProxyPassEndpointInfo,
) -> Result<Vec<ProxyPassLocation>, ProxyPassError> {
    let result = app
        .settings_reader
        .get_locations(app, endpoint_info)
        .await?;

    if result.len() == 0 {
        return Err(ProxyPassError::NoConfigurationFound);
    }

    if endpoint_info.debug {
        for location in &result {
            println!(
                "Request {} got locations: {}->{}",
                endpoint_info.as_str(),
                location.path,
                location.content_source.to_string()
            );
        }
    }

    Ok(result)
}
