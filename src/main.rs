use std::{sync::Arc, time::Duration};

use rust_extensions::MyTimer;
use timers::{CrlRefresherTimer, GcConnectionsTimer, MetricsTimer, SslCertsRefreshTimer};

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
mod settings_compiled;
mod tcp_listener;
mod tcp_utils;

mod scripts;

mod http2_client_pool;
mod http_client_pool;
mod http_clients;
mod metrics;
mod ssl;
mod tcp_gateway;
mod tcp_or_unix;
mod timers;
mod types;

pub fn to_hyper_error(e: std::convert::Infallible) -> String {
    e.to_string()
}

#[tokio::main]
async fn main() {
    my_tls::install_default_crypto_providers();
    crate::http_server::start();

    crate::flows::load_everything_from_settings().await;

    let mut my_timer = MyTimer::new(Duration::from_secs(3600));

    my_timer.register_timer("CRL Refresh", Arc::new(CrlRefresherTimer));

    my_timer.register_timer("SSL Certs Refresh", Arc::new(SslCertsRefreshTimer));

    my_timer.start(
        crate::app::APP_CTX.states.clone(),
        my_logger::LOGGER.clone(),
    );

    let mut metrics_timer = MyTimer::new(Duration::from_secs(1));

    metrics_timer.register_timer("Metrics", Arc::new(MetricsTimer));

    metrics_timer.start(
        crate::app::APP_CTX.states.clone(),
        my_logger::LOGGER.clone(),
    );

    let mut gc_connections_time = rust_extensions::MyTimer::new(Duration::from_secs(60));

    gc_connections_time.register_timer("GcConnections", Arc::new(GcConnectionsTimer));

    gc_connections_time.start(
        crate::app::APP_CTX.states.clone(),
        my_logger::LOGGER.clone(),
    );

    crate::app::APP_CTX.states.wait_until_shutdown().await;

    println!("Shutting down...");

    //    app.ssh_to_http_port_forward_pool.clean_up().await;

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    println!("Stopped...");
}

fn format_mem(size: usize) -> String {
    if size < 1024 {
        return format!("{}B", size);
    }

    let size = size as f64 / 1024.0;

    if size < 1024.0 {
        return format!("{:.2}KB", size);
    }

    let size = size as f64 / 1024.0;

    return format!("{:.2}Mb", size);
}
