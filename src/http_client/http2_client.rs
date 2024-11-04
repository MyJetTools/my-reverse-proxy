use my_http_client::http2::MyHttp2Client;
use tokio::net::TcpStream;

use my_tls::tokio_rustls::client::TlsStream;

use crate::configurations::RemoteHost;

use super::{HttpConnector, HttpTlsConnector};

pub enum Http2Client {
    Http(MyHttp2Client<TcpStream, HttpConnector>),
    Https(MyHttp2Client<TlsStream<tokio::net::TcpStream>, HttpTlsConnector>),
}

impl Http2Client {
    pub fn create(remote_host: RemoteHost, domain_name: Option<String>, debug: bool) -> Self {
        if remote_host.is_https() {
            let tls_stream = HttpTlsConnector {
                remote_host,
                domain_name,
                debug,
            };
            return Self::Https(MyHttp2Client::new(tls_stream));
        }

        let http_connector = HttpConnector { remote_host, debug };
        return Self::Http(MyHttp2Client::new(http_connector));
    }
}
