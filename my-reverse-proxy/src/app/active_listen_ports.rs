use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use crate::tcp_listener::ListenServerHandler;

pub struct ActiveListenPorts {
    pub tcp: HashMap<u16, Arc<ListenServerHandler>>,
    pub unix: HashMap<Arc<String>, Arc<ListenServerHandler>>,
}

impl ActiveListenPorts {
    pub fn new() -> Self {
        Self {
            tcp: HashMap::new(),
            unix: HashMap::new(),
        }
    }

    pub fn kick_tcp_if_needed(&mut self, port: u16) {
        if self.tcp.contains_key(&port) {
            return;
        }

        println!("Starting server on port {}", port);
        let listen_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
        let server_handler = crate::tcp_listener::start_listen_tcp_server(listen_addr);

        self.tcp.insert(port, server_handler);
    }

    pub fn kick_unix_if_needed(&mut self, host: Arc<String>) {
        if self.unix.contains_key(&host) {
            return;
        }

        println!("Starting unix socket server '{}'", host);

        let server_handler = crate::tcp_listener::start_listen_unix_server(host.clone());

        self.unix.insert(host, server_handler);
    }
}
