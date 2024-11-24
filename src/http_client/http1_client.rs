use std::sync::Arc;

use my_tls::tokio_rustls::client::TlsStream;
use tokio::net::TcpStream;

use my_http_client::http1::MyHttpClient;

use crate::{app::Prometheus, configurations::*};

use super::{HttpConnector, HttpTlsConnector};

pub enum Http1Client {
    Http(MyHttpClient<TcpStream, HttpConnector>),
    Https(MyHttpClient<TlsStream<tokio::net::TcpStream>, HttpTlsConnector>),
}

impl Http1Client {
    pub fn create(
        prometheus: Arc<Prometheus>,
        remote_host: RemoteHost,
        domain_name: Option<String>,
        debug: bool,
    ) -> Self {
        if remote_host.is_https() {
            let tls_stream = HttpTlsConnector {
                remote_host,
                domain_name,
                debug,
            };
            return Self::Https(MyHttpClient::new_with_metrics(tls_stream, prometheus));
        }

        let http_connector = HttpConnector { remote_host, debug };
        return Self::Http(MyHttpClient::new_with_metrics(http_connector, prometheus));
    }
}
