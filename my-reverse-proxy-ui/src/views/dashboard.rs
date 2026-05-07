use std::time::Duration;

use dioxus::prelude::*;
use dioxus_utils::{js::sleep, DataState, RenderState};

use crate::{
    api,
    models::{
        CurrentConfigurationModel, HttpEndpointInfoModel, HttpProxyPassLocationModel,
        PortConfigurationModel,
    },
};

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

    let state_ra = state.read();

    match state_ra.as_ref() {
        RenderState::None | RenderState::Loading => {
            rsx! { div { class: "loading", "Loading configuration..." } }
        }
        RenderState::Error(err) => rsx! {
            div { class: "error",
                h3 { "Failed to load configuration" }
                pre { "{err}" }
            }
        },
        RenderState::Loaded(cfg) => render_dashboard(cfg),
    }
}

fn render_dashboard(cfg: &CurrentConfigurationModel) -> Element {
    rsx! {
        div { class: "dashboard",
            h2 { "Reverse Proxy" }
            for port in &cfg.ports {
                {render_port(port)}
            }
            if !cfg.ip_lists.is_empty() {
                {render_ip_lists(cfg)}
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

fn render_port(port: &PortConfigurationModel) -> Element {
    let port_type_class = format!("type-pill listen-{}", normalize_type(port.r#type.as_str()));

    rsx! {
        section { class: "port",
            div { class: "port-header",
                span { class: "label", "Port" }
                span { class: "number", "{port.port}" }
                span { class: "{port_type_class}", "{port.r#type}" }
                span { class: "conn-count",
                    span { class: "label", "TCP" }
                    span { class: "value", "{port.inbound_connections}" }
                }
            }
            div { class: "endpoints",
                for endpoint in &port.endpoints {
                    {render_endpoint(endpoint)}
                }
            }
        }
    }
}

fn render_endpoint(endpoint: &HttpEndpointInfoModel) -> Element {
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
                        }
                    }
                    tbody {
                        for loc in &endpoint.locations {
                            {render_location(loc)}
                        }
                    }
                }
            }
        }
    }
}

fn render_location(loc: &HttpProxyPassLocationModel) -> Element {
    let pool_label = match (loc.pool_alive, loc.pool_total) {
        (Some(alive), Some(total)) => format!("{alive}/{total}"),
        _ => "—".to_string(),
    };

    let type_class = format!("type-pill type-{}", normalize_type(loc.r#type.as_str()));

    rsx! {
        tr {
            td { "{loc.path}" }
            td {
                span { class: "{type_class}", "{loc.r#type}" }
            }
            td { class: "upstream", "{loc.to}" }
            td { "{loc.location_id}" }
            td { "{pool_label}" }
            td { class: "id-string", "{loc.id_string}" }
        }
    }
}

fn normalize_type(t: &str) -> String {
    // CSS-class friendly slug: lowercase, `+` → `-`.
    t.replace('+', "-").to_ascii_lowercase()
}
