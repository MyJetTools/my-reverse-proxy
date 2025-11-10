use std::{net::SocketAddr, sync::Arc};

#[derive(Clone)]
pub enum ListenHost {
    Tcp(SocketAddr),
    Unix(Arc<String>),
}

impl Into<ListenHost> for Arc<String> {
    fn into(self) -> ListenHost {
        ListenHost::Unix(self)
    }
}

impl ListenHost {
    pub fn to_pretty_string(&self, is_https: bool) -> String {
        match self {
            ListenHost::Tcp(socket_addr) => {
                if is_https {
                    format!("https://{}", socket_addr)
                } else {
                    format!("http://{}", socket_addr)
                }
            }
            ListenHost::Unix(socket) => socket.to_string(),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ListenHost::Tcp(socket_addr) => socket_addr.to_string(),
            ListenHost::Unix(socket) => socket.to_string(),
        }
    }

    pub fn get_port(&self) -> Option<u16> {
        match self {
            ListenHost::Tcp(socket_addr) => Some(socket_addr.port()),
            ListenHost::Unix(_) => None,
        }
    }
}

impl Into<ListenHost> for SocketAddr {
    fn into(self) -> ListenHost {
        ListenHost::Tcp(self)
    }
}
