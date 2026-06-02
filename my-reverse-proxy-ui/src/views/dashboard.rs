use std::time::Duration;

use dioxus::prelude::*;
use dioxus_utils::{js::sleep, DataState, RenderState};

use crate::{
    api,
    models::{
        CurrentConfigurationModel, GatewayClientStatusModel, GatewayConnectionModel,
        GatewayServerStatusModel, HttpEndpointInfoModel, HttpProxyPassLocationModel,
        PortConfigurationModel, SslCertificateInfoModel,
    },
    views::{LogScope, LogsDialog, LogsDialogRequest},
};

type LogsDialogSignal = Signal<Option<LogsDialogRequest>>;

const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[component]
pub fn Dashboard() -> Element {
    let mut state = use_signal(DataState::<CurrentConfigurationModel>::new);

    // Start a single background poll loop on mount. Re-renders driven by
    // `state.write()` won't restart it.
    use_hook(|| {
        spawn(async move {
            loop {
                match api::get_current_configuration().await {
                    Ok(data) => state.write().set_value(data),
                    Err(err) => state.write().set_error(err),
                }
                sleep(REFRESH_INTERVAL).await;
            }
        });
    });

    let logs_dialog = use_signal(|| Option::<LogsDialogRequest>::None);

    let state_ra = state.read();

    let content = match state_ra.as_ref() {
        RenderState::None | RenderState::Loading => {
            rsx! { div { class: "loading", "Loading configuration..." } }
        }
        RenderState::Error(err) => rsx! {
            div { class: "error",
                h3 { "Failed to load configuration" }
                pre { "{err}" }
            }
        },
        RenderState::Loaded(cfg) => render_dashboard(cfg, logs_dialog),
    };

    let open_dialog = logs_dialog.read().clone();

    rsx! {
        {content}
        if let Some(request) = open_dialog {
            LogsDialog { request, dialog: logs_dialog }
        }
    }
}

fn render_dashboard(cfg: &CurrentConfigurationModel, dialog: LogsDialogSignal) -> Element {
    rsx! {
        div { class: "dashboard",
            h2 { "Reverse Proxy" }
            for port in &cfg.ports {
                {render_port(port, dialog)}
            }
            if let Some(server) = cfg.gateway_server.as_ref() {
                {render_gateway_server(server)}
            }
            if !cfg.gateway_clients.is_empty() {
                {render_gateway_clients(&cfg.gateway_clients)}
            }
            if !cfg.ip_lists.is_empty() {
                {render_ip_lists(cfg)}
            }
            if !cfg.ssl_certs.is_empty() {
                {render_ssl_certs(&cfg.ssl_certs)}
            }
            if !cfg.errors.is_empty() {
                section { class: "port",
                    div { class: "port-header",
                        span { class: "label", "Errors" }
                    }
                    div { class: "endpoints",
                        ul {
                            for (host, err) in &cfg.errors {
                                li { b { "{host}" } " — " span { "{err}" } }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_ssl_certs(certs: &[SslCertificateInfoModel]) -> Element {
    rsx! {
        section { class: "port",
            div { class: "port-header",
                span { class: "label", "SSL certificates" }
                span { class: "number", "{certs.len()}" }
            }
            div { class: "locations-wrap ssl-table-wrap",
                table { class: "locations",
                    thead {
                        tr {
                            th { "Cert ID" }
                            th { "Days left" }
                            th { "Expires at" }
                        }
                    }
                    tbody {
                        for c in certs {
                            {render_ssl_cert(c)}
                        }
                    }
                }
            }
        }
    }
}

fn render_ssl_cert(c: &SslCertificateInfoModel) -> Element {
    let pill_class = if c.days_left < 7 {
        "days-left critical"
    } else if c.days_left < 30 {
        "days-left warn"
    } else {
        "days-left ok"
    };

    rsx! {
        tr {
            td { class: "id-string", "{c.id}" }
            td {
                span { class: "{pill_class}", "{c.days_left}" }
            }
            td { class: "id-string", "{c.expires_at}" }
        }
    }
}

fn render_gateway_server(server: &GatewayServerStatusModel) -> Element {
    rsx! {
        section { class: "port",
            div { class: "port-header",
                span { class: "label", "Gateway Server" }
                span { class: "number", "{server.connections.len()}" }
                {
                    let conn_class = if server.connections.is_empty() {
                        "conn-count"
                    } else {
                        "conn-count active"
                    };
                    rsx! {
                        span { class: "{conn_class}",
                            span { class: "label", "Clients" }
                            span { class: "value", "{server.connections.len()}" }
                        }
                    }
                }
            }
            if server.connections.is_empty() {
                div { class: "endpoints",
                    div { class: "gateway-empty", "No gateway clients connected" }
                }
            } else {
                {render_gateway_connections(&server.connections)}
            }
        }
    }
}

fn render_gateway_clients(clients: &[GatewayClientStatusModel]) -> Element {
    rsx! {
        section { class: "port",
            div { class: "port-header",
                span { class: "label", "Gateway Clients" }
                span { class: "number", "{clients.len()}" }
            }
            div { class: "endpoints",
                for client in clients {
                    div { class: "endpoint",
                        div { class: "endpoint-header",
                            span { class: "host", "{client.name}" }
                            {
                                let conn_class = if client.connections.is_empty() {
                                    "conn-count endpoint-conn"
                                } else {
                                    "conn-count endpoint-conn active"
                                };
                                rsx! {
                                    span { class: "{conn_class}",
                                        span { class: "label", "Conn" }
                                        span { class: "value", "{client.connections.len()}" }
                                    }
                                }
                            }
                        }
                        if client.connections.is_empty() {
                            div { class: "gateway-empty", "Not connected" }
                        } else {
                            {render_gateway_connections(&client.connections)}
                        }
                    }
                }
            }
        }
    }
}

fn render_gateway_connections(connections: &[GatewayConnectionModel]) -> Element {
    rsx! {
        div { class: "locations-wrap",
            table { class: "locations",
                thead {
                    tr {
                        th { "Gateway" }
                        th { "Forward conn" }
                        th { "Proxy conn" }
                        th { "Ping" }
                        th { "In/s" }
                        th { "Out/s" }
                        th { "Incoming forward" }
                        th { "Connected at" }
                    }
                }
                tbody {
                    for conn in connections {
                        {render_gateway_connection(conn)}
                    }
                }
            }
        }
    }
}

fn render_gateway_connection(conn: &GatewayConnectionModel) -> Element {
    let (forward_label, forward_class) = if conn.is_incoming_forward_connection_allowed {
        ("allowed", "type-pill type-http2")
    } else {
        ("blocked", "type-pill type-drop")
    };

    let route_rows = gateway_route_rows(conn);

    let in_per_sec = fmt_bytes_per_sec(conn.in_history.last().copied().unwrap_or(0));
    let out_per_sec = fmt_bytes_per_sec(conn.out_history.last().copied().unwrap_or(0));

    rsx! {
        tr {
            td { class: "id-string", "{conn.name}" }
            td { "{conn.forward_connections}" }
            td { "{conn.proxy_connections}" }
            td { "{conn.ping_time}" }
            td { "{in_per_sec}" }
            td { "{out_per_sec}" }
            td {
                span { class: "{forward_class}", "{forward_label}" }
            }
            td { class: "id-string", "{conn.timestamp}" }
        }
        if !route_rows.is_empty() {
            tr { class: "gateway-routes-row",
                td { colspan: "8",
                    table { class: "locations gateway-routes",
                        thead {
                            tr {
                                th { "Route" }
                                th { "Forward" }
                                th { "Proxy" }
                            }
                        }
                        tbody {
                            for (route, fwd, proxy) in route_rows.iter() {
                                tr {
                                    td { class: "id-string", "{route}" }
                                    td { "{fwd}" }
                                    td { "{proxy}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Union of forward/proxy route keys, sorted, as (route, forward_count, proxy_count).
fn gateway_route_rows(conn: &GatewayConnectionModel) -> Vec<(String, usize, usize)> {
    let mut keys: Vec<&String> = conn
        .forward_routes
        .keys()
        .chain(conn.proxy_routes.keys())
        .collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|k| {
            (
                k.clone(),
                conn.forward_routes.get(k).copied().unwrap_or(0),
                conn.proxy_routes.get(k).copied().unwrap_or(0),
            )
        })
        .collect()
}

fn render_ip_lists(cfg: &CurrentConfigurationModel) -> Element {
    rsx! {
        section { class: "port",
            div { class: "port-header",
                span { class: "label", "IP whitelists" }
                span { class: "number", "{cfg.ip_lists.len()}" }
            }
            div { class: "ip-lists",
                for (id, ips) in &cfg.ip_lists {
                    div { class: "ip-list",
                        div { class: "ip-list-header",
                            span { class: "id", "{id}" }
                            span { class: "count", "{ips.len()}" }
                        }
                        if ips.is_empty() {
                            div { class: "empty", "(empty)" }
                        } else {
                            div { class: "ip-entries",
                                for ip in ips {
                                    span { class: "ip-entry", "{ip}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_port(port: &PortConfigurationModel, dialog: LogsDialogSignal) -> Element {
    let port_type_class = format!("type-pill listen-{}", normalize_type(port.r#type.as_str()));

    let port_conn_class = if port.inbound_connections > 0 {
        "conn-count active"
    } else {
        "conn-count"
    };

    let (logs_id, logs_title) = match port.unix_socket.as_ref() {
        Some(socket) => (socket.clone(), format!("Port logs — unix {socket}")),
        None => (port.port.to_string(), format!("Port logs — {}", port.port)),
    };

    rsx! {
        section { class: "port",
            div { class: "port-header",
                if let Some(socket) = port.unix_socket.as_ref() {
                    span { class: "label", "Unix socket" }
                    span { class: "id-string", "{socket}" }
                    span { class: "{port_type_class}", "{port.r#type}" }
                } else {
                    span { class: "label", "Port" }
                    span { class: "number", "{port.port}" }
                    span { class: "{port_type_class}", "{port.r#type}" }
                    span { class: "{port_conn_class}",
                        span { class: "label", "TCP" }
                        span { class: "value", "{port.inbound_connections}" }
                    }
                }
                {render_logs_button(dialog, logs_title, LogScope::Port(logs_id))}
            }
            div { class: "endpoints",
                for endpoint in &port.endpoints {
                    {render_endpoint(endpoint, dialog)}
                }
            }
        }
    }
}

/// A small "logs" button that opens the in-memory logs dialog for a given scope.
fn render_logs_button(
    mut dialog: LogsDialogSignal,
    title: String,
    scope: LogScope,
) -> Element {
    rsx! {
        button {
            class: "logs-btn",
            onclick: move |_| dialog.set(Some(LogsDialogRequest { title: title.clone(), scope: scope.clone() })),
            "logs"
        }
    }
}

fn render_endpoint(endpoint: &HttpEndpointInfoModel, dialog: LogsDialogSignal) -> Element {
    let listen_type_class = format!("type-pill listen-{}", normalize_type(endpoint.r#type.as_str()));

    let has_meta = endpoint.ssl_cert_id.is_some()
        || endpoint.client_cert_id.is_some()
        || endpoint.g_auth.is_some()
        || endpoint.allowed_user_list_id.is_some()
        || endpoint.ip_list.is_some();

    rsx! {
        div { class: "endpoint",
            div { class: "endpoint-header",
                span { class: "{listen_type_class}", "{endpoint.r#type}" }
                span { class: "host", "{endpoint.host}" }
                if endpoint.debug {
                    span { class: "debug-badge", "debug" }
                }
                {
                    let endpoint_conn_class = if endpoint.inbound_connections > 0 {
                        "conn-count endpoint-conn active"
                    } else {
                        "conn-count endpoint-conn"
                    };
                    rsx! {
                        span { class: "{endpoint_conn_class}",
                            span { class: "label", "TCP" }
                            span { class: "value", "{endpoint.inbound_connections}" }
                        }
                    }
                }
                {render_logs_button(dialog, format!("Endpoint logs — {}", endpoint.host), LogScope::Endpoint(endpoint.host.clone()))}
            }
            if has_meta {
                div { class: "endpoint-meta",
                    if let Some(ssl) = endpoint.ssl_cert_id.as_ref() {
                        span { class: "meta-chip ssl",
                            span { class: "label", "SSL" }
                            span { class: "value", "{ssl}" }
                        }
                    }
                    if let Some(client) = endpoint.client_cert_id.as_ref() {
                        span { class: "meta-chip auth client",
                            span { class: "label", "Client-Cert" }
                            span { class: "value", "{client}" }
                        }
                    }
                    if let Some(auth) = endpoint.g_auth.as_ref() {
                        span { class: "meta-chip auth google",
                            span { class: "label", "Google-Auth" }
                            span { class: "value", "{auth}" }
                        }
                    }
                    if let Some(users) = endpoint.allowed_user_list_id.as_ref() {
                        span { class: "meta-chip auth users",
                            span { class: "label", "Users" }
                            span { class: "value", "{users}" }
                        }
                    }
                    if let Some(ip) = endpoint.ip_list.as_ref() {
                        span { class: "meta-chip ip",
                            span { class: "label", "IP-List" }
                            span { class: "value", "{ip}" }
                        }
                    }
                }
            }
            div { class: "locations-wrap",
                table { class: "locations",
                    thead {
                        tr {
                            th { "Path" }
                            th { "Type" }
                            th { "Upstream" }
                            th { "Loc id" }
                            th { "Pool" }
                            th { "id_string" }
                            th { "Logs" }
                        }
                    }
                    tbody {
                        for loc in &endpoint.locations {
                            {render_location(loc, dialog)}
                        }
                    }
                }
            }
        }
    }
}

fn render_location(loc: &HttpProxyPassLocationModel, dialog: LogsDialogSignal) -> Element {
    let pool_label = match (loc.pool_alive, loc.pool_total) {
        (Some(alive), Some(total)) => format!("{alive}/{total}"),
        _ => "—".to_string(),
    };

    let type_class = format!("type-pill type-{}", normalize_type(loc.r#type.as_str()));

    let row_class = match loc.last_status {
        Some(1) => "upstream-ok",
        Some(2) => "upstream-error",
        Some(0) => "upstream-unknown",
        _ => "",
    };

    let logs_title = format!("Location logs — {} ({})", loc.path, loc.location_id);

    rsx! {
        tr { class: "{row_class}",
            td { "{loc.path}" }
            td {
                span { class: "{type_class}", "{loc.r#type}" }
            }
            td { class: "upstream", "{loc.to}" }
            td { "{loc.location_id}" }
            td { "{pool_label}" }
            td { class: "id-string", "{loc.id_string}" }
            td {
                {render_logs_button(dialog, logs_title, LogScope::Location(loc.location_id))}
            }
        }
    }
}

fn fmt_bytes_per_sec(bytes: usize) -> String {
    let v = bytes as f64;
    if v >= 1_048_576.0 {
        format!("{:.1} MB/s", v / 1_048_576.0)
    } else if v >= 1024.0 {
        format!("{:.1} KB/s", v / 1024.0)
    } else {
        format!("{} B/s", bytes)
    }
}

fn normalize_type(t: &str) -> String {
    // CSS-class friendly slug: lowercase, `+` → `-`.
    t.replace('+', "-").to_ascii_lowercase()
}
