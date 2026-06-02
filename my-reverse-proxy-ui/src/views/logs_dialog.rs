use dioxus::prelude::*;
use dioxus_utils::{DataState, RenderState};

use crate::models::ProxyLogsModel;

/// Which in-memory log buffer to show. Mirrors the three axes of the proxy.
#[derive(Clone, PartialEq)]
pub enum LogScope {
    Port(String),
    Endpoint(String),
    Location(i64),
}

/// What the dashboard asks the dialog to display. Stored in a `Signal<Option<_>>`
/// at the Dashboard level; `None` means the dialog is closed.
#[derive(Clone, PartialEq)]
pub struct LogsDialogRequest {
    pub title: String,
    pub scope: LogScope,
}

#[component]
pub fn LogsDialog(
    request: LogsDialogRequest,
    dialog: Signal<Option<LogsDialogRequest>>,
) -> Element {
    let mut data = use_signal(DataState::<ProxyLogsModel>::new);
    let title = request.title.clone();

    let data_ra = data.read();

    let body = match data_ra.as_ref() {
        RenderState::None => {
            let scope = request.scope.clone();
            spawn(async move {
                data.write().set_loading();
                let result = match scope {
                    LogScope::Port(id) => crate::api::get_port_logs(&id).await,
                    LogScope::Endpoint(id) => crate::api::get_endpoint_logs(&id).await,
                    LogScope::Location(id) => crate::api::get_location_logs(id).await,
                };
                match result {
                    Ok(value) => data.write().set_value(value),
                    Err(err) => data.write().set_error(err),
                }
            });
            rsx! { div { class: "logs-empty", "Loading logs..." } }
        }
        RenderState::Loading => rsx! { div { class: "logs-empty", "Loading logs..." } },
        RenderState::Error(err) => rsx! { div { class: "logs-error", "{err}" } },
        RenderState::Loaded(logs) => {
            if logs.items.is_empty() {
                rsx! { div { class: "logs-empty", "No log messages yet" } }
            } else {
                rsx! {
                    div { class: "logs-lines",
                        for line in &logs.items {
                            div { class: "logs-line",
                                span { class: "logs-time", "{fmt_time(line.moment)}" }
                                span { class: "logs-msg", "{line.message}" }
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        div {
            class: "logs-overlay",
            onclick: move |_| dialog.set(None),
            div {
                class: "logs-modal",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "logs-modal-header",
                    span { class: "logs-modal-title", "{title}" }
                    button {
                        class: "logs-close",
                        onclick: move |_| dialog.set(None),
                        "✕"
                    }
                }
                div { class: "logs-modal-body", {body} }
            }
        }
    }
}

/// Format unix-microseconds as a `HH:MM:SS.mmm` UTC time-of-day. Avoids pulling
/// in a date library for a debug log view.
fn fmt_time(micros: i64) -> String {
    if micros <= 0 {
        return "—".to_string();
    }
    let total_millis = micros / 1000;
    let millis = (total_millis % 1000) as u64;
    let total_secs = (total_millis / 1000) as u64;
    let secs_of_day = total_secs % 86_400;
    let h = secs_of_day / 3600;
    let m = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}
