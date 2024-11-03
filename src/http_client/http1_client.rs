use std::sync::Arc;

use my_tls::tokio_rustls::client::TlsStream;
use rust_extensions::StrOrString;
use tokio::net::TcpStream;

use my_http_client::{MyHttpClient, MyHttpClientConnector, MyHttpClientError};

use crate::configurations::*;

pub enum Http1Client {
    Http(MyHttpClient<TcpStream, Http1Connector>),
    Https(MyHttpClient<TlsStream<tokio::net::TcpStream>, Http1TlsConnector>),
}

impl Http1Client {
    pub fn create(remote_host: RemoteHost, domain_name: Option<String>, debug: bool) -> Self {
        if remote_host.is_https() {
            let tls_stream = Http1TlsConnector {
                remote_host,
                domain_name,
                debug,
            };
            return Http1Client::Https(MyHttpClient::new(tls_stream));
        }

        let http1_connector = Http1Connector { remote_host, debug };
        return Http1Client::Http(MyHttpClient::new(http1_connector));
    }
}

pub struct Http1Connector {
    remote_host: RemoteHost,
    debug: bool,
}

#[async_trait::async_trait]
impl MyHttpClientConnector<TcpStream> for Http1Connector {
    fn get_remote_host(&self) -> StrOrString {
        self.remote_host.as_str().into()
    }

    fn is_debug(&self) -> bool {
        self.debug
    }

    async fn connect(&self) -> Result<TcpStream, MyHttpClientError> {
        match TcpStream::connect(self.remote_host.get_host_port()).await {
            Ok(tcp_stream) => Ok(tcp_stream),
            Err(err) => Err(
                my_http_client::MyHttpClientError::CanNotConnectToRemoteHost(format!(
                    "{}. Err:{}",
                    self.remote_host.as_str(),
                    err
                )),
            ),
        }
    }
}

pub struct Http1TlsConnector {
    remote_host: RemoteHost,
    domain_name: Option<String>,
    debug: bool,
}

#[async_trait::async_trait]
impl MyHttpClientConnector<TlsStream<TcpStream>> for Http1TlsConnector {
    fn get_remote_host(&self) -> StrOrString {
        self.remote_host.as_str().into()
    }

    fn is_debug(&self) -> bool {
        self.debug
    }

    async fn connect(&self) -> Result<TlsStream<TcpStream>, my_http_client::MyHttpClientError> {
        use my_tls::tokio_rustls::rustls::pki_types::ServerName;

        let host_port = self.remote_host.get_host_port();

        let tcp_stream = if host_port.find(":").is_none() {
            match TcpStream::connect(format!("{}:443", self.remote_host.get_host_port())).await {
                Ok(tcp_stream) => tcp_stream,
                Err(err) => {
                    return Err(
                        my_http_client::MyHttpClientError::CanNotConnectToRemoteHost(format!(
                            "{}",
                            err
                        )),
                    )
                }
            }
        } else {
            match TcpStream::connect(host_port).await {
                Ok(tcp_stream) => tcp_stream,
                Err(err) => {
                    return Err(
                        my_http_client::MyHttpClientError::CanNotConnectToRemoteHost(format!(
                            "{}",
                            err
                        )),
                    )
                }
            }
        };

        if self.debug {
            println!(
                "Connecting to TLS remote host: {}",
                self.remote_host.get_host_port(),
            );
        }

        let config = my_tls::tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(my_tls::ROOT_CERT_STORE.clone())
            .with_no_client_auth();

        let connector = my_tls::tokio_rustls::TlsConnector::from(Arc::new(config));
        let domain = if let Some(domain_name) = self.domain_name.as_ref() {
            ServerName::try_from(domain_name.to_string()).unwrap()
        } else {
            ServerName::try_from(self.remote_host.get_host().to_string()).unwrap()
        };

        if self.debug {
            println!("TLS Domain Name: {:?}", domain);
        }

        let tls_stream = connector
            .connect_with(domain, tcp_stream, |itm| {
                if self.debug {
                    println!("Debugging: {:?}", itm.alpn_protocol());
                }
            })
            .await;

        let tls_stream = match tls_stream {
            Ok(tls_stream) => tls_stream,
            Err(err) => {
                return Err(
                    my_http_client::MyHttpClientError::CanNotConnectToRemoteHost(format!(
                        "{}",
                        err
                    )),
                )
            }
        };

        return Ok(tls_stream);
    }
}
