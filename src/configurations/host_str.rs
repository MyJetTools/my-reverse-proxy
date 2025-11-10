use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use crate::types::ListenHost;

#[derive(Clone)]
pub enum EndpointPort {
    Tcp(u16),
    UnixSocket(Arc<String>),
}

#[derive(Clone)]
pub struct EndpointHttpHostString {
    src: Arc<String>,
    port: Option<u16>,
    index: Option<usize>,
}

impl EndpointHttpHostString {
    pub fn new(host: String) -> Result<Self, String> {
        let is_unix_socket = host.starts_with('/') || host.starts_with("~/");

        if is_unix_socket {
            return Ok(Self {
                src: Arc::new(host),
                port: None,
                index: None,
            });
        }

        let index = host.find(':');

        let port_str = match index {
            Some(index) => &host[index + 1..],
            None => host.as_str(),
        };

        let port: u16 = match port_str.parse() {
            Ok(result) => result,
            Err(_) => {
                return Err(format!("Can not pars endpoint port for host: {}", host));
            }
        };

        let result = Self {
            src: Arc::new(host),
            port: Some(port),
            index,
        };

        Ok(result)
    }

    pub fn has_server_name(&self) -> bool {
        self.index.is_some()
    }

    pub fn get_server_name(&self) -> Option<&str> {
        let index = self.index?;
        let result = &self.src[..index];
        Some(result)
    }

    pub fn is_my_server_name(&self, server_name: &str) -> bool {
        match self.get_server_name() {
            Some(my_server_name) => my_server_name.eq_ignore_ascii_case(server_name),
            None => true,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.src
    }

    pub fn get_port(&self) -> EndpointPort {
        match self.port {
            Some(port) => EndpointPort::Tcp(port),
            None => EndpointPort::UnixSocket(self.src.clone()),
        }
    }

    pub fn is_unix_socket(&self) -> bool {
        self.port.is_none()
    }

    pub fn get_listen_host(&self) -> ListenHost {
        match self.get_port() {
            EndpointPort::Tcp(port) => {
                let socket_addr =
                    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);

                return ListenHost::Tcp(socket_addr);
            }
            EndpointPort::UnixSocket(host) => ListenHost::Unix(host),
        }
    }
}
