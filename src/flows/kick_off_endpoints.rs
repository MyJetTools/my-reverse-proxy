use std::sync::Arc;

use crate::app::AppContext;

pub async fn kick_off_endpoints(app: &Arc<AppContext>) {
    let app_configuration = app.current_app_configuration.read().await;
    for (listen_port, port_configuration) in &app_configuration.as_ref().unwrap().http_endpoints {
        let listen_end_point = std::net::SocketAddr::new([0, 0, 0, 0].into(), *listen_port);

        if port_configuration.is_https() {
            crate::http_server::start_https_server(listen_end_point, app.clone());
        } else if port_configuration.is_http1() {
            crate::http_server::start_http_server(listen_end_point, app.clone());
        } else {
            crate::http_server::start_h2_server(listen_end_point, app.clone());
        }
    }

    for (listen_port, port_configuration) in &app_configuration.as_ref().unwrap().tcp_endpoints {
        let listen_end_point = std::net::SocketAddr::new([0, 0, 0, 0].into(), *listen_port);

        crate::tcp_port_forward::start_tcp(
            app.clone(),
            listen_end_point,
            port_configuration.clone(),
        );
    }

    for (listen_port, port_configuration) in
        &app_configuration.as_ref().unwrap().tcp_over_ssh_endpoints
    {
        let listen_end_point = std::net::SocketAddr::new([0, 0, 0, 0].into(), *listen_port);

        crate::tcp_port_forward::start_tcp_over_ssh(
            app.clone(),
            listen_end_point,
            port_configuration.clone(),
        );
    }
}
