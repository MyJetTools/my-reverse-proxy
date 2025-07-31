use std::sync::Arc;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use my_http_client::MyHttpClientDisconnect;
use my_ssh::SshAsyncChannel;

use crate::tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream;

pub enum HttpResponse {
    Response(my_http_client::HyperResponse),
    WebSocketUpgrade {
        stream: WebSocketUpgradeStream,
        response: hyper::Response<BoxBody<Bytes, String>>,
        disconnection: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static>,
    },
}

pub enum WebSocketUpgradeStream {
    TcpStream(tokio::net::TcpStream),
    UnixStream(tokio::net::UnixStream),
    TlsStream(my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>),
    SshChannel(SshAsyncChannel),
    HttpOverGatewayStream(TcpGatewayProxyForwardStream),
    // Hyper(HyperWebsocket),
}
