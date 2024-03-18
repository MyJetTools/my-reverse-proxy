use std::sync::Arc;

use app::AppContext;

mod app;
mod flows;
mod http_client;
mod http_server;
mod settings;
mod ssh_configuration;

#[tokio::main]
async fn main() {
    let settings_reader = settings::SettingsReader::new(".my-reverse-proxy").await;
    let app = AppContext::new(settings_reader);

    let app = Arc::new(app);

    let port = std::env::var("LISTEN_PORT").unwrap_or("8000".to_owned());

    let port = port.parse::<u16>().unwrap();

    let http_server =
        http_server::HttpServer::new(std::net::SocketAddr::from(([0, 0, 0, 0], port)));

    http_server.start(app);

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
