use crate::{app::AppContext, http_proxy_pass::*};

pub async fn get_locations<'s>(
    app: &AppContext,
    endpoint_info: &HttpServerConnectionInfo,
    req: &HttpRequestBuilder,
) -> Result<(Vec<ProxyPassLocation>, Option<AllowedUserList>), ProxyPassError> {
    let (result, allowed_users_list) = app
        .settings_reader
        .get_locations(app, req, endpoint_info.http_type.is_https())
        .await?;

    if result.len() == 0 {
        let scheme = if endpoint_info.http_type.is_https() {
            "https"
        } else {
            "http"
        };
        println!(
            "Request {scheme}://{} has no locations to serve",
            req.get_host_port(),
        );
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

    Ok((result, allowed_users_list))
}
