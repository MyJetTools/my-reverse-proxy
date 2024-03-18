use std::sync::Arc;

use app::AppContext;

mod app;
mod flows;
mod http_client;
mod http_server;
mod settings;

mod tcp_port_forward;

#[tokio::main]
async fn main() {
    let settings_reader = settings::SettingsReader::new(".my-reverse-proxy").await;

    let listen_ports = settings_reader.get_listen_ports().await;
    let app = AppContext::new(settings_reader);

    let app = Arc::new(app);

    for (listen_port, endpoint_type) in listen_ports {
        let listen_end_point = std::net::SocketAddr::from(([0, 0, 0, 0], listen_port));

        match endpoint_type {
            settings::EndpointType::Http1 => {
                let http_server = http_server::HttpServer::new(listen_end_point);

                http_server.start(app.clone());
            }
            settings::EndpointType::Tcp(remote_addr) => {
                crate::tcp_port_forward::start_tcp(listen_end_point, remote_addr);
            }
            settings::EndpointType::TcpOverSsh(ssh_configuration) => {
                crate::tcp_port_forward::start_tcp_over_ssh(listen_end_point, ssh_configuration);
            }
        }
    }

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
