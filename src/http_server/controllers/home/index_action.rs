use std::{collections::BTreeMap, sync::Arc};

use my_http_server::{
    macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType,
};
use rust_extensions::{date_time::DateTimeAsMicroseconds, StrOrString};

use crate::{
    app::AppContext, configurations::*,
    http_server::controllers::configuration::contracts::CurrentConfigurationHttpModel,
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
    let config = CurrentConfigurationHttpModel::new(&action.app).await;
    HttpOutput::Content {
        headers: None,
        content_type: WebContentType::Html.into(),
        content: create_html_content(&action.app, config).await.into_bytes(),
    }
    .into_ok_result(false)
}

async fn create_html_content(
    app: &AppContext,
    config_model: CurrentConfigurationHttpModel,
) -> String {
    let mut table_lines = String::new();

    const ERR_STYLE: &str = "color: white;background: red;font-weight: 500;";
    for (host, error) in config_model.errors.iter() {
        table_lines.push_str(
            format!(r##"<tr><td style="{ERR_STYLE}">{host}</td><td colspan="4" style="{ERR_STYLE}">{error}</td></tr>"##)
                .as_str(),
        );
    }

    for port_configuration in &config_model.ports {
        let connections = app
            .metrics
            .get(|itm| itm.connection_by_port.get(&port_configuration.port))
            .await;

        let class = if connections > 0 {
            "text-bg-success"
        } else {
            "text-bg-secondary"
        };
        let mut draw_port = format!("<span class='badge {class}' style='border-radius: 5px 0 0 5px;'>{connections}</span><span class='badge text-bg-secondary'  style='border-radius: 0 5px 5px 0;'>{}</span>", port_configuration.port);

        for http_endpoint in &port_configuration.endpoints {
            let allowed_users_html =
                if let Some(allowed_user_list_id) = &http_endpoint.allowed_user_list_id {
                    let mut allowed_users_html = String::new();
                    allowed_users_html.push_str("<div>");

                    allowed_users_html.push_str(
                        format!(
                            r##"<span class="badge text-bg-success">{allowed_user_list_id}</span>"##
                        )
                        .as_str(),
                    );

                    allowed_users_html.push_str("</div>");

                    allowed_users_html
                } else {
                    "".to_string()
                };

            let mut locations_html = String::new();
            for location in &http_endpoint.locations {
                let proxy_pass_to = location.to.to_string();
                let remote_type = render_http_badge(&location.r#type);
                let remote_type = remote_type.as_str();
                let path = location.path.as_str();
                locations_html.push_str(
                        format!(r##"<div><span class="badge text-bg-secondary">{path}</span> → {remote_type}<span class="badge text-bg-secondary" style="{RIGHT_BADGE_STYLE}">{proxy_pass_to}</span></div>"##,).as_str(),
                    );
            }

            let host = http_endpoint.host.as_str();

            let host_type = render_http_badge(&http_endpoint.r#type);
            let host_type = host_type.as_str();

            let debug = if http_endpoint.debug {
                r##"<span class="badge text-bg-warning" style="border-radius: 0;">debug</span>"##
            } else {
                ""
            };

            let now = DateTimeAsMicroseconds::now();

            let ssl_cert = if let Some(ssl_cert) = &http_endpoint.ssl_cert_id {
                let ssl_cert = ssl_cert.as_str();
                if ssl_cert == crate::self_signed_cert::SELF_SIGNED_CERT_NAME {
                    format!(r##"<span class="badge text-bg-warning">{ssl_cert}</span>"##)
                } else {
                    let ssl_cert_ref = SslCertificateIdRef::new(ssl_cert);
                    let cert = app
                        .ssl_certificates_cache
                        .read(|inner| inner.ssl_certs.get(ssl_cert_ref).clone())
                        .await;

                    match cert {
                        Some(holder) => {
                            let cert_info = holder.ssl_cert.get_cert_info().await;

                            let expires = cert_info.expires.duration_since(now);
                            let badge_type = match expires {
                                rust_extensions::date_time::DateTimeDuration::Positive(_) => {
                                    "text-bg-success"
                                }
                                _ => "text-bg-danger",
                            };
                            format!(
                                r##"<span class="badge {badge_type}">{ssl_cert} expires: {:?}</span>"##,
                                expires
                            )
                        }
                        None => {
                            format!(r##"<span class="badge text-bg-danger">{ssl_cert}</span>"##)
                        }
                    }
                }
            } else {
                "-".to_string()
            };

            let auth: StrOrString = if let Some(client_ssl_cert) = &http_endpoint.client_cert_id {
                format!(
                    "<span class='badge text-bg-success'>CS: {}</span>",
                    client_ssl_cert.as_str()
                )
                .into()
            } else {
                if let Some(ga) = &http_endpoint.g_auth {
                    format!("<span class='badge text-bg-success'>GA: {ga}</span>").into()
                } else {
                    "-".into()
                }
            };

            let auth = auth.as_str();

            table_lines.push_str(
                    format!(
                        r##"<tr><td>{draw_port}</td><td>{host_type}{debug}<span class="badge text-bg-secondary" style="{RIGHT_BADGE_STYLE}">{host}</span>  {allowed_users_html}</td><td>{ssl_cert}</td><td>{auth}</td><td>{locations_html}</td></tr>"##,
                    )
                    .as_str(),
                );

            draw_port = "".to_string();
        }
    }

    let mut users = String::new();
    render_users(&mut users, &config_model.users);
    render_ip_list(&mut users, &config_model.ip_lists);

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
                <th>Auth</th>
                <th>Location</th>
            </tr>
            {table_lines}
            </table>

            {users}

        </body>
        "##
    )
}

fn render_http_badge(src: &str) -> StrOrString<'static> {
    match src {
        "http" => {
            r##"<span class="badge text-bg-warning" style="border-radius: 5px 0 0 5px;">http1</span>"##.into()
        }
        "http1" => {
            r##"<span class="badge text-bg-warning" style="border-radius: 5px 0 0 5px;">http1</span>"##.into()
        }
        "http2" => {
            r##"<span class="badge text-bg-info" style="border-radius: 5px 0 0 5px;">http2</span>"##.into()
        }
        "https" => {
            r##"<span class="badge text-bg-primary" style="border-radius: 5px 0 0 5px;">https1</span>"##.into()
        }
        "https1" => {
            r##"<span class="badge text-bg-primary" style="border-radius: 5px 0 0 5px;">https1</span>"##.into()
        }
        "https2" => {
            r##"<span class="badge text-bg-success" style="border-radius: 5px 0 0 5px;">https2</span>"##.into()
        }
        "files_path" => {
            r##"<span class="badge text-bg-info" style="border-radius: 5px 0 0 5px;">files</span>"##.into()
        }
        _ => {
            format!(
                r##"<span class="badge text-bg-danger" style="border-radius: 5px 0 0 5px;">{src}</span>"##
            ).into()
        }
    }
}

fn render_users(html: &mut String, users: &BTreeMap<String, Vec<String>>) {
    html.push_str("<h3>Users</h3>");
    html.push_str(r#"<table class="table table-striped" style="width:100%;"><tr><th>Id</th><th>Users</th><tr>"#);

    for (id, list) in users {
        html.push_str(format!(r#"<tr><td>{id}</td><td>"#).as_str());

        for user in list {
            html.push_str(
                format!(r#"<span class="badge text-bg-secondary">{user}</span>"#).as_str(),
            );
        }

        html.push_str("</td></tr>");
    }

    html.push_str("</table>");
}

fn render_ip_list(html: &mut String, ip_lists: &BTreeMap<String, Vec<String>>) {
    html.push_str("<h3>Ip White Lists</h3>");
    html.push_str(r#"<table class="table table-striped" style="width:100%;"><tr><th>Id</th><th>Ip list</th><tr>"#);

    for (id, list) in ip_lists {
        html.push_str(format!(r#"<tr><td>{id}</td><td>"#).as_str());

        for user in list {
            html.push_str(
                format!(r#"<span class="badge text-bg-secondary">{user}</span>"#).as_str(),
            );
        }

        html.push_str("</td></tr>");
    }

    html.push_str("</table>");
}
