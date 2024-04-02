use std::sync::Arc;

use crate::{app::AppContext, app_configuration::HttpEndpointInfo, http_proxy_pass::*};

//todo!("delete")
pub async fn get_locations<'s>(
    app: &AppContext,
    endpoint_info: &HttpEndpointInfo,
    req: &HttpRequestBuilder,
) -> Result<(Vec<ProxyPassLocation>, Option<Arc<AllowedUserList>>), ProxyPassError> {
    let read_access = app.current_app_configuration.read().await;

    let result =
        read_access.get_http_locations(endpoint_info, req, endpoint_info.http_type.is_https());

    match result {
        Ok(result) => {
            if endpoint_info.debug {
                for location in &result.0 {
                    println!(
                        "Request {} got locations: {}->{}",
                        endpoint_info.as_str(),
                        location.config.path,
                        location.content_source.to_string()
                    );
                }
            }

            Ok(result)
        }
        Err(e) => Err(ProxyPassError::CanNotReadSettingsConfiguration(e)),
    }
}
