use std::sync::Arc;

use crate::app::AppContext;

pub async fn kick_off_endpoints(app: &Arc<AppContext>) {
    let app_configuration = app.get_current_app_configuration().await;
    for (listen_port, port_configuration) in &app_configuration.http_endpoints {
        let listen_end_point = std::net::SocketAddr::new([0, 0, 0, 0].into(), *listen_port);

        if port_configuration.is_https() {
            crate::http_server::start_https_server(
                listen_end_point,
                app.clone(),
                port_configuration.debug,
            );
        } else if port_configuration.is_http1() {
            crate::http_server::start_http_server(
                listen_end_point,
                app.clone(),
                port_configuration.debug,
            );
        } else {
            crate::http_server::start_h2_server(
                listen_end_point,
                app.clone(),
                port_configuration.debug,
            );
        }
    }

    for (listen_port, port_configuration) in &app_configuration.tcp_endpoints {
        let listen_end_point = std::net::SocketAddr::new([0, 0, 0, 0].into(), *listen_port);

        crate::tcp_port_forward::start_tcp(
            app.clone(),
            listen_end_point,
            port_configuration.clone(),
        );
    }

    for (listen_port, port_configuration) in &app_configuration.tcp_over_ssh_endpoints {
        let listen_end_point = std::net::SocketAddr::new([0, 0, 0, 0].into(), *listen_port);

        crate::tcp_port_forward::start_tcp_over_ssh(
            app.clone(),
            listen_end_point,
            port_configuration.clone(),
        );
    }
}
