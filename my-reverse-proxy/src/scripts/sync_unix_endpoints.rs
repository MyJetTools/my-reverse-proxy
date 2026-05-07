use std::sync::Arc;

pub async fn sync_unix_endpoints() {
    let unix_ports_to_be_listened = crate::app::APP_CTX
        .current_configuration
        .get(|config| {
            let result: Vec<_> = config
                .listen_unix_socket_endpoints
                .keys()
                .map(|host| host.clone())
                .collect();
            result
        })
        .await;

    sync_unix_ports(unix_ports_to_be_listened).await;
}

async fn sync_unix_ports(ports_to_be_listened: Vec<Arc<String>>) {
    println!("Hosts to be listened: {:?}", ports_to_be_listened);

    let mut listen_end_points = crate::app::APP_CTX.active_listen_ports.lock().await;

    for host_to_be_listened in &ports_to_be_listened {
        listen_end_points.kick_unix_if_needed(host_to_be_listened.clone());
    }

    let mut ports_to_stop = Vec::new();

    for currently_active_port in listen_end_points.unix.keys() {
        if !ports_to_be_listened.contains(currently_active_port) {
            ports_to_stop.push(currently_active_port.clone());
        }
    }

    for port_to_stop in ports_to_stop {
        if let Some(server_handler) = listen_end_points.unix.remove(&port_to_stop) {
            println!("Stopping server on port {}", port_to_stop);
            server_handler.stop().await;
            println!("Stopped server on port {}", port_to_stop);
        }
    }
}
