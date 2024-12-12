use std::{sync::Arc, time::Duration};

use app::AppContext;

use timers::{CrlRefresherTimer, GcConnectionsTimer, SslCertsRefreshTimer};

mod app;
mod flows;
//mod http2_executor;

mod configurations;
mod consts;
//mod crl;
mod google_auth;
mod http_client_connectors;
mod http_content_source;
mod http_proxy_pass;
mod http_server;
mod self_signed_cert;
mod settings;
mod tcp_listener;
//mod ssh_to_http_port_forward;

mod scripts;

mod http2_client_pool;
mod http_client_pool;
mod ssl;
mod timers;
mod types;

pub fn to_hyper_error(e: std::convert::Infallible) -> String {
    e.to_string()
}

#[tokio::main]
async fn main() {
    my_tls::install_default_crypto_providers();
    let settings_model = settings::SettingsModel::load(".my-reverse-proxy")
        .await
        .unwrap();

    let control_port = settings_model.get_http_control_port();

    let app = AppContext::new(settings_model);

    let app = Arc::new(app);

    crate::http_server::start(&app, control_port);

    crate::flows::load_everything_from_settings(&app).await;

    let mut my_timer = rust_extensions::MyTimer::new(Duration::from_secs(3600));

    my_timer.register_timer("CRL Refresh", Arc::new(CrlRefresherTimer::new(app.clone())));

    my_timer.register_timer(
        "SSL Certs Refresh",
        Arc::new(SslCertsRefreshTimer::new(app.clone())),
    );

    my_timer.start(app.states.clone(), my_logger::LOGGER.clone());

    let mut gc_connections_time = rust_extensions::MyTimer::new(Duration::from_secs(60 * 3));

    gc_connections_time.register_timer(
        "GcConnections",
        Arc::new(GcConnectionsTimer::new(app.clone())),
    );

    gc_connections_time.start(app.states.clone(), my_logger::LOGGER.clone());

    app.states.wait_until_shutdown().await;

    println!("Shutting down...");

    //    app.ssh_to_http_port_forward_pool.clean_up().await;

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Stopped...");
}
