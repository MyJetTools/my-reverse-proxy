use std::sync::Arc;

use crate::{
    configurations::{HttpEndpointInfo, HttpListenPortConfiguration},
    tcp_listener::https::ClientCertificateData,
    types::*,
};

#[derive(Clone)]
pub struct HttpConnectionInfo {
    pub cn_user_name: Option<Arc<ClientCertificateData>>,
    pub connection_ip: ConnectionIp,
    pub endpoint_info: Option<Arc<HttpEndpointInfo>>,
    pub listen_config: Arc<HttpListenPortConfiguration>,
}

pub fn kick_h1_reverse_proxy_server(
    connection_ip: ConnectionIp,
    endpoint_info: Arc<HttpEndpointInfo>,
    server_stream: my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    cn_user_name: Option<Arc<ClientCertificateData>>,
    listen_config: Arc<HttpListenPortConfiguration>,
) {
    let _ = connection_ip;
    let http_connection_info = HttpConnectionInfo {
        connection_ip,
        cn_user_name,
        endpoint_info: Some(endpoint_info),
        listen_config,
    };
    tokio::spawn(super::server_loop::serve_reverse_proxy(
        server_stream,
        http_connection_info,
    ));
}

pub fn kick_h1_tcp_reverse_proxy_server_from_http(
    accepted_connection: tokio::net::TcpStream,
    connection_ip: impl Into<ConnectionIp>,
    listen_config: Arc<HttpListenPortConfiguration>,
) {
    let http_connection_info = HttpConnectionInfo {
        connection_ip: connection_ip.into(),
        cn_user_name: None,
        endpoint_info: None,
        listen_config,
    };

    tokio::spawn(super::server_loop::serve_reverse_proxy(
        accepted_connection,
        http_connection_info,
    ));
}

pub fn kick_h1_unix_reverse_proxy_server_from_http(
    accepted_connection: tokio::net::UnixStream,
    listen_config: Arc<HttpListenPortConfiguration>,
) {
    let http_connection_info = HttpConnectionInfo {
        connection_ip: ConnectionIp::UnixSocket,
        cn_user_name: None,
        endpoint_info: None,
        listen_config,
    };
    tokio::spawn(super::server_loop::serve_reverse_proxy(
        accepted_connection,
        http_connection_info,
    ));
}
