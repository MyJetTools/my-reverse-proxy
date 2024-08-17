use std::{sync::Arc, time::Duration};

use app::AppContext;
use flows::kick_off_endpoints;
use timers::CrlRefresherTimer;

mod app;
mod flows;
//mod http2_executor;
mod configurations;
mod crl;
mod files_cache;
mod google_auth;
mod http_client;
mod http_content_source;
mod http_control;
mod http_proxy_pass;
mod http_server;
mod populate_variable;
mod self_signed_cert;
mod settings;
mod ssh_to_http_port_forward_pool;
mod ssl;
mod tcp_port_forward;
mod timers;
mod types;
mod variables_reader;

pub fn to_hyper_error(e: std::convert::Infallible) -> String {
    e.to_string()
}

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

    let mut my_timer = rust_extensions::MyTimer::new(Duration::from_secs(60));

    my_timer.register_timer("CRL Refresh", Arc::new(CrlRefresherTimer::new(app.clone())));

    my_timer.start(app.states.clone(), my_logger::LOGGER.clone());

    app.states.wait_until_shutdown().await;

    println!("Shutting down...");

    app.ssh_to_http_port_forward_pool.clean_up().await;

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Stopped...");
}
