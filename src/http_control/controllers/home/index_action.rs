use std::sync::Arc;

use my_http_server::{
    macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType,
};

use crate::{app::AppContext, app_configuration::AppConfiguration};

#[http_route(
    method: "GET",
    route: "/",
)]
pub struct IndexAction {
    app: Arc<AppContext>,
}

impl IndexAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &IndexAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let config = action.app.get_current_app_configuration().await;

    HttpOutput::Content {
        headers: None,
        content_type: WebContentType::Html.into(),
        content: create_html_content(config.as_ref()).into_bytes(),
    }
    .into_ok_result(false)
}

fn create_html_content(config: &AppConfiguration) -> String {
    let mut table_lines = String::new();
    for (port, config) in &config.http_endpoints {
        table_lines.push_str(
            format!(r##"<tr style="background:black; color:white;"><td>{port}</td><td colspan="4"></td></tr>"##)
                .as_str(),
        );

        for http_endpoint in &config.endpoint_info {
            let allowed_users_html = if let Some(allowed_user_list) =
                &http_endpoint.allowed_user_list
            {
                let mut allowed_users_html = String::new();
                allowed_users_html.push_str("<div>");
                for user in allowed_user_list.get_list() {
                    allowed_users_html.push_str(
                        format!(r##"<span class="badge text-bg-success">{user}</span>"##).as_str(),
                    );
                }
                allowed_users_html.push_str("</div>");

                allowed_users_html
            } else {
                "".to_string()
            };

            let mut locations_html = String::new();
            for location in &http_endpoint.locations {
                let proxy_pass_to = location.get_proxy_pass_to_as_string();
                let remote_type = location.remote_type.to_str();
                let path = location.path.as_str();
                locations_html.push_str(
                    format!(r##"<div>'{path}' -> [{remote_type}]{proxy_pass_to}</div>"##,).as_str(),
                );
            }

            let host = http_endpoint.host_endpoint.as_str();

            let host_type = http_endpoint.http_type.to_str();

            let ssl_cert = if let Some(ssl_cert) = &http_endpoint.ssl_certificate_id {
                ssl_cert.as_str()
            } else {
                "-"
            };

            let client_ssl_cert =
                if let Some(client_ssl_cert) = &http_endpoint.client_certificate_id {
                    client_ssl_cert.as_str()
                } else {
                    "-"
                };

            table_lines.push_str(
                format!(
                    r##"<tr><td></td><td>[{host_type}]{host} {allowed_users_html}</td><td>{ssl_cert}</td><td>{client_ssl_cert}</td><td>{locations_html}</td></tr>"##,
                )
                .as_str(),
            );
        }
    }

    format!(
        r##"
<!DOCTYPE html>
<html>
    <head>
        <title>MyReverseProxy</title>

        <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/css/bootstrap.min.css" rel="stylesheet" integrity="sha384-QWTKZyjpPEjISv5WaRU9OFeRpok6YctnYmDr5pNlyT2bRjXh0JMhjY6hW+ALEwIH" crossorigin="anonymous">
    </head>
    <body>
        <h1>Http configs</h1>
        <table class="table table-striped" style="width:100%;">
        <tr>
            <th>Port</th>
            <th>Host</th>
            <th>Ssl cert</th>
            <th>Client Ssl</th>
            <th>Location</th>
        </tr>
        {table_lines}
        </table>
      
    </body>          
    "##
    )
}
