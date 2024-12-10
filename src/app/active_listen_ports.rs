use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use crate::tcp_listener::ListenServerHandler;

use super::AppContext;

pub struct ActiveListenPorts {
    pub data: HashMap<u16, Arc<ListenServerHandler>>,
}

impl ActiveListenPorts {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn kick_it_if_needed(&mut self, port: u16, app: &Arc<AppContext>) {
        if self.data.contains_key(&port) {
            return;
        }
        let listen_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);
        let server_handler = crate::tcp_listener::start_listen_server(listen_addr, app.clone());

        self.data.insert(port, server_handler);
    }
}
