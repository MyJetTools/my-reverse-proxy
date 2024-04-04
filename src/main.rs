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
    let settings_reader = settings::SettingsReader::new(".my-reverse-proxy").await;

    let app = AppContext::new(settings_reader).await;

    let app = Arc::new(app);

    let app_configuration = crate::flows::get_and_check_app_config(&app).await.unwrap();

    app.current_app_configuration
        .write()
        .await
        .replace(app_configuration);

    kick_off_endpoints(&app).await;

    app.states.wait_until_shutdown().await;
}
