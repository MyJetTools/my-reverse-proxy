use std::sync::Arc;

use my_http_server::{
    macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType,
};

use crate::{
    app::AppContext,
    app_configuration::{AppConfiguration, HttpType, SELF_SIGNED_CERT_NAME},
};

const RIGHT_BADGE_STYLE: &str = "border-radius: 0 5px 5px 0;";

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
        let mut draw_port = port.to_string();

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
                let remote_type = render_http_badge(location.remote_type);
                let path = location.path.as_str();
                locations_html.push_str(
                    format!(r##"<div><span class="badge text-bg-secondary">{path}</span> → {remote_type}<span class="badge text-bg-secondary" style="{RIGHT_BADGE_STYLE}">{proxy_pass_to}</span></div>"##,).as_str(),
                );
            }

            let host = http_endpoint.host_endpoint.as_str();

            let host_type = render_http_badge(http_endpoint.http_type);

            let ssl_cert = if let Some(ssl_cert) = &http_endpoint.ssl_certificate_id {
                let ssl_cert = ssl_cert.as_str();
                if ssl_cert == SELF_SIGNED_CERT_NAME {
                    format!(r##"<span class="badge text-bg-warning">{ssl_cert}</span>"##)
                } else {
                    format!(r##"<span class="badge text-bg-success">{ssl_cert}</span>"##)
                }
            } else {
                "-".to_string()
            };

            let client_ssl_cert =
                if let Some(client_ssl_cert) = &http_endpoint.client_certificate_id {
                    client_ssl_cert.as_str()
                } else {
                    "-"
                };

            table_lines.push_str(
                format!(
                    r##"<tr><td>{draw_port}</td><td>{host_type}<span class="badge text-bg-secondary" style="{RIGHT_BADGE_STYLE}">{host}</span> {allowed_users_html}</td><td>{ssl_cert}</td><td>{client_ssl_cert}</td><td>{locations_html}</td></tr>"##,
                )
                .as_str(),
            );

            draw_port = "".to_string();
        }
    }

    format!(
        r##"
<!DOCTYPE html>
<html>
    <head>
        <title>MyReverseProxy</title>
        <meta http-equiv="content-type" content="text/html; charset=utf-8">
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

fn render_http_badge(src: HttpType) -> &'static str {
    match src {
        HttpType::Http1 => {
            r##"<span class="badge text-bg-warning" style="border-radius: 5px 0 0 5px;">http1</span>"##
        }
        HttpType::Http2 => {
            r##"<span class="badge text-bg-info" style="border-radius: 5px 0 0 5px;">http2</span>"##
        }
        HttpType::Https1 => {
            r##"<span class="badge text-bg-primary" style="border-radius: 5px 0 0 5px;">https1</span>"##
        }
        HttpType::Https2 => {
            r##"<span class="badge text-bg-success" style="border-radius: 5px 0 0 5px;">https2</span>"##
        }
    }
}
