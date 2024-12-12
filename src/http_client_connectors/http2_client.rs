use my_http_client::http2::MyHttp2Client;
use rust_extensions::remote_endpoint::{RemoteEndpointOwned, Scheme};
use tokio::net::TcpStream;

use my_tls::tokio_rustls::client::TlsStream;

use crate::app::AppContext;

use super::{HttpConnector, HttpTlsConnector};

pub enum Http2Client {
    Http(MyHttp2Client<TcpStream, HttpConnector>),
    Https(MyHttp2Client<TlsStream<tokio::net::TcpStream>, HttpTlsConnector>),
}

impl Http2Client {
    pub fn create(
        app: &AppContext,
        remote_endpoint: RemoteEndpointOwned,
        domain_name: Option<String>,
        debug: bool,
    ) -> Self {
        let is_https = if let Some(scheme) = remote_endpoint.get_scheme() {
            match scheme {
                Scheme::Http => false,
                Scheme::Https => true,
                Scheme::UnixSocket => {
                    panic!("UnixSocket is not supported for HTTP2");
                }
            }
        } else {
            panic!(
                "Scheme is not set for remote resource {}",
                remote_endpoint.as_str()
            );
        };

        if is_https {
            let tls_stream = HttpTlsConnector {
                remote_endpoint,
                domain_name,
                debug,
            };
            return Self::Https(MyHttp2Client::new(tls_stream, app.prometheus.clone()));
        }

        let http_connector = HttpConnector {
            remote_endpoint,
            debug,
        };
        return Self::Http(MyHttp2Client::new(http_connector, app.prometheus.clone()));
    }
}
