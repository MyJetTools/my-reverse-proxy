use std::collections::BTreeMap;

use rust_extensions::{date_time::DateTimeAsMicroseconds, StrOrString};

use crate::{
    configurations::SslCertificateIdRef,
    http_server::controllers::configuration::contracts::{
        CurrentConfigurationHttpModel, GatewayClientStatus, GatewayServerStatus,
    },
};

const RIGHT_BADGE_STYLE: &str = "border-radius: 0 5px 5px 0;";

pub async fn render_content(config_model: &CurrentConfigurationHttpModel) -> String {
    let mut table_lines = String::new();

    const ERR_STYLE: &str = "color: white;background: red;font-weight: 500;";
    for (host, error) in config_model.errors.iter() {
        table_lines.push_str(
            format!(r##"<tr><td style="{ERR_STYLE}">{host}</td><td colspan="4" style="{ERR_STYLE}">{error}</td></tr>"##)
                .as_str(),
        );
    }

    for port_configuration in &config_model.ports {
        let connections = crate::app::APP_CTX
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
            let ip_list = if let Some(ip_list) = http_endpoint.ip_list.as_ref() {
                let ip_list = ip_list.as_str();
                let shield_icon = super::icons::shield_icon();
                format!(r##"<span class="badge text-bg-success">{shield_icon} {ip_list}</span>"##)
            } else {
                "".into()
            };

            let allowed_users_html = if let Some(allowed_user_list_id) =
                &http_endpoint.allowed_user_list_id
            {
                let mut allowed_users_html = String::new();
                allowed_users_html.push_str("<div>");

                let users_icon = super::icons::users_icon();

                allowed_users_html.push_str(
                    format!(
                        r##"<span class="badge text-bg-success">{users_icon}{allowed_user_list_id}</span>"##
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

                let connections_amount = if let Some(count) =
                    config_model.remote_connections.get(proxy_pass_to.as_str())
                {
                    format!(
                        r##"<span class="badge text-bg-success" style="border-radius:0">{count}</span>"##,
                    )
                } else {
                    String::new()
                };

                locations_html.push_str(
                        format!(r##"<div><span class="badge text-bg-secondary">{path}</span> → {remote_type}{connections_amount}<span class="badge text-bg-secondary" style="{RIGHT_BADGE_STYLE}">{proxy_pass_to}</span></div>"##,).as_str(),
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
                    let cert = crate::app::APP_CTX
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
                        r##"<tr><td>{draw_port}</td><td>{host_type}{debug}<span class="badge text-bg-secondary" style="{RIGHT_BADGE_STYLE}">{host}</span><div>{ip_list}</div></td><td>{ssl_cert}</td><td>{auth} {allowed_users_html}</td><td>{locations_html}</td></tr>"##,
                    )
                    .as_str(),
                );

            draw_port = "".to_string();
        }
    }

    let mut users = String::new();
    render_users(&mut users, &config_model.users);
    render_ip_list(&mut users, &config_model.ip_lists);

    let mut gateways = String::new();
    render_server_gateway(&mut gateways, config_model.gateway_server.as_ref());
    render_client_gateway(&mut gateways, config_model.gateway_clients.as_slice());

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
            {gateways}
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

fn render_server_gateway(html: &mut String, gateway_server_status: Option<&GatewayServerStatus>) {
    if let Some(gateway_server_status) = gateway_server_status {
        html.push_str("<h1>GATEWAY SERVER</h1>");

        html.push_str("<table  class=\"table table-striped\"><thead><tr><th>Connection name</th><th>Forward connections</th><th>Proxy connections</th><th>Ping time</th><th>In</th><th>Out</th></tr></thead><tbody>");

        for connection in gateway_server_status.connections.as_slice() {
            html.push_str(
                format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    connection.name.as_str(),
                    connection.forward_connections,
                    connection.proxy_connections,
                    connection.ping_time,
                    super::render_graph(connection.in_history.as_slice()),
                    super::render_graph(connection.out_history.as_slice())
                )
                .as_str(),
            );
        }

        html.push_str("</tbody></table>");
    }
}

fn render_client_gateway(html: &mut String, gateway_client_status: &[GatewayClientStatus]) {
    if gateway_client_status.len() == 0 {
        return;
    }

    html.push_str("<h1>GATEWAY CLIENTS</h1>");

    html.push_str("<table  class=\"table table-striped\"><thead><tr><th>Connection name</th><th>Forward connections</th><th>Proxy connections</th><th>Ping time</th><th>In</th><th>Out</th></tr></thead><tbody>");

    for gateway_client in gateway_client_status {
        for connection in gateway_client.connections.as_slice() {
            let forward_connections: StrOrString =
                if connection.is_incoming_forward_connection_allowed {
                    connection.forward_connections.to_string().into()
                } else {
                    "Not Allowed".into()
                };

            html.push_str(
                format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    connection.name.as_str(),
                    forward_connections,
                    connection.proxy_connections,
                    connection.ping_time,
                    super::render_graph(connection.in_history.as_slice()),
                    super::render_graph(connection.out_history.as_slice())
                )
                .as_str(),
            );
        }
    }

    html.push_str("</tbody></table>");
}
