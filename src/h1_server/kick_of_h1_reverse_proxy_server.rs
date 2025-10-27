use std::{net::SocketAddr, sync::Arc};

use crate::{
    configurations::{HttpEndpointInfo, HttpListenPortConfiguration},
    network_stream::*,
    tcp_listener::{https::ClientCertificateData, AcceptedTcpConnection},
};

pub fn kick_h1_reverse_proxy_server(
    endpoint_name: Arc<String>,
    socket_addr: SocketAddr,
    endpoint_info: Arc<HttpEndpointInfo>,
    server_stream: my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    cn_user_name: Option<Arc<ClientCertificateData>>,
    listen_config: Arc<HttpListenPortConfiguration>,
) {
    tokio::spawn(super::server_loop::serve_reverse_proxy(
        server_stream,
        Some(endpoint_info),
        listen_config,
    ));
}

pub fn kick_h1_reverse_proxy_server_from_http(
    server_stream: AcceptedTcpConnection,
    listen_config: Arc<HttpListenPortConfiguration>,
) {
    match server_stream.network_stream {
        MyNetworkStream::Tcp(tcp_stream) => {
            tokio::spawn(super::server_loop::serve_reverse_proxy(
                tcp_stream,
                None,
                listen_config,
            ));
        }
        MyNetworkStream::UnixSocket(unix_stream) => {
            tokio::spawn(super::server_loop::serve_reverse_proxy(
                unix_stream,
                None,
                listen_config,
            ));
        }
        MyNetworkStream::Ssh(async_channel) => {
            tokio::spawn(super::server_loop::serve_reverse_proxy(
                async_channel,
                None,
                listen_config,
            ));
        }
    }
}
