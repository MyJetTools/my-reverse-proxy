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

    let listen_ports = settings_reader.get_listen_ports().await;
    let app = AppContext::new(settings_reader);

    let app = Arc::new(app);

    for listen_port in listen_ports {
        let http_server =
            http_server::HttpServer::new(std::net::SocketAddr::from(([0, 0, 0, 0], listen_port)));

        http_server.start(app.clone());
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
