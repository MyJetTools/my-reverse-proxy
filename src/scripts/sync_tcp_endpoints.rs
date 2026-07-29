pub async fn sync_endpoints() {
    sync_tcp_endpoints().await;
    super::sync_unix_endpoints().await;
}

async fn sync_tcp_endpoints() {
    let tcp_ports_to_be_listened = crate::app::APP_CTX
        .current_configuration
        .get(|config| {
            let result: Vec<_> = config
                .listen_tcp_endpoints
                .keys()
                .map(|port| *port)
                .collect();
            result
        })
        .await;

    sync_tcp_ports(tcp_ports_to_be_listened).await;
}

async fn sync_tcp_ports(ports_to_be_listened: Vec<u16>) {
    println!("Ports ro be listened: {:?}", ports_to_be_listened);

    let mut listen_end_points = crate::app::APP_CTX.active_listen_ports.lock().await;

    for port_to_be_listened in &ports_to_be_listened {
        listen_end_points.kick_tcp_if_needed(*port_to_be_listened);
    }

    let mut ports_to_stop = Vec::new();

    for currently_active_port in listen_end_points.tcp.keys() {
        if !ports_to_be_listened.contains(currently_active_port) {
            ports_to_stop.push(*currently_active_port);
        }
    }

    for port_to_stop in ports_to_stop {
        if let Some(server_handler) = listen_end_points.tcp.remove(&port_to_stop) {
            println!("Stopping server on port {}", port_to_stop);
            server_handler.stop().await;
            println!("Stopped server on port {}", port_to_stop);
        }
    }
}
