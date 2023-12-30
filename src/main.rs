use std::sync::Arc;

use app::AppContext;

mod app;
mod http_client;
mod http_server;
mod settings;

#[tokio::main]
async fn main() {
    let settings_reader = settings::SettingsReader::new(".my-reverse-proxy").await;
    let app = AppContext::new(settings_reader);

    let app = Arc::new(app);

    let http_server =
        http_server::HttpServer::new(std::net::SocketAddr::from(([0, 0, 0, 0], 9000)));

    http_server.start(app);

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
