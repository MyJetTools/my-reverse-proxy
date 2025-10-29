use std::{net::SocketAddr, sync::Arc};

use crate::{
    configurations::{HttpEndpointInfo, HttpListenPortConfiguration},
    network_stream::*,
    tcp_listener::{https::ClientCertificateData, AcceptedTcpConnection},
};

#[derive(Clone)]
pub struct HttpConnectionInfo {
    pub cn_user_name: Option<Arc<ClientCertificateData>>,
    pub socket_addr: SocketAddr,
    pub listening_addr: SocketAddr,
    pub endpoint_info: Option<Arc<HttpEndpointInfo>>,
    pub listen_config: Arc<HttpListenPortConfiguration>,
}

pub fn kick_h1_reverse_proxy_server(
    listening_addr: SocketAddr,
    socket_addr: SocketAddr,
    endpoint_info: Arc<HttpEndpointInfo>,
    server_stream: my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    cn_user_name: Option<Arc<ClientCertificateData>>,
    listen_config: Arc<HttpListenPortConfiguration>,
) {
    let http_connection_info = HttpConnectionInfo {
        socket_addr,
        cn_user_name,
        listening_addr,
        endpoint_info: Some(endpoint_info),
        listen_config,
    };
    tokio::spawn(super::server_loop::serve_reverse_proxy(
        server_stream,
        http_connection_info,
    ));
}

pub fn kick_h1_reverse_proxy_server_from_http(
    listening_addr: SocketAddr,
    server_stream: AcceptedTcpConnection,
    listen_config: Arc<HttpListenPortConfiguration>,
) {
    let http_connection_info = HttpConnectionInfo {
        socket_addr: server_stream.addr,
        cn_user_name: None,
        listening_addr,
        endpoint_info: None,
        listen_config,
    };
    match server_stream.network_stream {
        MyNetworkStream::Tcp(tcp_stream) => {
            tokio::spawn(super::server_loop::serve_reverse_proxy(
                tcp_stream,
                http_connection_info,
            ));
        }
        MyNetworkStream::UnixSocket(unix_stream) => {
            tokio::spawn(super::server_loop::serve_reverse_proxy(
                unix_stream,
                http_connection_info,
            ));
        }
        MyNetworkStream::Ssh(async_channel) => {
            tokio::spawn(super::server_loop::serve_reverse_proxy(
                async_channel,
                http_connection_info,
            ));
        }
    }
}
