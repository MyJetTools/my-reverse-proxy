use std::sync::Arc;

use app::AppContext;
use flows::kick_off_endpoints;

mod app;
mod flows;
//mod http2_executor;
mod app_configuration;
mod google_auth;
mod http_client;
mod http_content_source;
mod http_control;
mod http_proxy_pass;
mod http_server;
mod populate_variable;
mod self_signed_cert;
mod settings;
mod ssl;
mod tcp_port_forward;
mod types;

#[tokio::main]
async fn main() {
    let settings_model = settings::SettingsModel::load(".my-reverse-proxy")
        .await
        .unwrap();

    let app = AppContext::new(settings_model);

    let app = Arc::new(app);

    let app_configuration = crate::flows::get_and_check_app_config(&app).await.unwrap();

    app.set_current_app_configuration(app_configuration).await;

    kick_off_endpoints(&app).await;

    crate::http_control::start(&app);

    app.states.wait_until_shutdown().await;
}
