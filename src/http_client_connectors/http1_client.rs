use std::sync::Arc;

use my_tls::tokio_rustls::client::TlsStream;
use rust_extensions::remote_endpoint::RemoteEndpointOwned;
use tokio::net::TcpStream;

use my_http_client::http1::MyHttpClient;

use crate::app::Prometheus;

use super::{HttpConnector, HttpTlsConnector};

pub enum Http1Client {
    Http(MyHttpClient<TcpStream, HttpConnector>),
    Https(MyHttpClient<TlsStream<tokio::net::TcpStream>, HttpTlsConnector>),
}

impl Http1Client {
    pub fn create(
        prometheus: Arc<Prometheus>,
        remote_endpoint: RemoteEndpointOwned,
        domain_name: Option<String>,
        debug: bool,
    ) -> Self {
        let is_https = if let Some(scheme) = remote_endpoint.get_scheme() {
            match scheme {
                rust_extensions::remote_endpoint::Scheme::Http => false,
                rust_extensions::remote_endpoint::Scheme::Https => true,
                rust_extensions::remote_endpoint::Scheme::UnixSocket => {
                    panic!(
                        "Http or Https scheme is expected within host: {}",
                        remote_endpoint.as_str()
                    );
                }
            }
        } else {
            panic!("Remote host scheme is not set {}", remote_endpoint.as_str());
        };

        if is_https {
            let tls_stream = HttpTlsConnector {
                remote_endpoint,
                domain_name,
                debug,
            };
            return Self::Https(MyHttpClient::new_with_metrics(tls_stream, prometheus));
        }

        let http_connector = HttpConnector {
            remote_endpoint,
            debug,
        };
        return Self::Http(MyHttpClient::new_with_metrics(http_connector, prometheus));
    }
}
