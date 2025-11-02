pub async fn sync_tcp_endpoints() {
    let ports_to_be_listened = crate::app::APP_CTX
        .current_configuration
        .get(|config| {
            let result: Vec<_> = config.listen_endpoints.keys().map(|port| *port).collect();
            result
        })
        .await;

    println!("Ports ro be listened: {:?}", ports_to_be_listened);

    let mut listen_end_points = crate::app::APP_CTX.active_listen_ports.lock().await;

    for port_to_be_listened in &ports_to_be_listened {
        listen_end_points.kick_it_if_needed(*port_to_be_listened);
    }

    let mut ports_to_stop = Vec::new();

    for currently_active_port in listen_end_points.data.keys() {
        if !ports_to_be_listened.contains(currently_active_port) {
            ports_to_stop.push(*currently_active_port);
        }
    }

    for port_to_stop in ports_to_stop {
        if let Some(server_handler) = listen_end_points.data.remove(&port_to_stop) {
            println!("Stopping server on port {}", port_to_stop);
            server_handler.stop().await;
            println!("Stopped server on port {}", port_to_stop);
        }
    }
}
