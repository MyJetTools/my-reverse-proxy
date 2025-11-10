use std::{sync::Arc, time::Duration};

use my_tls::tokio_rustls::{rustls::server::Acceptor, LazyConfigAcceptor};

use crate::{
    configurations::{HttpEndpointInfo, HttpListenPortConfiguration},
    tcp_listener::https::ClientCertificateData,
};

const RESOLVE_TLS_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn lazy_accept_tcp_stream(
    endpoint_port: u16,
    tcp_stream: tokio::net::TcpStream,
    configuration: Arc<HttpListenPortConfiguration>,
) -> Result<
    (
        my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        Arc<HttpEndpointInfo>,
        Option<Arc<ClientCertificateData>>,
    ),
    String,
> {
    let future = lazy_accept_tcp_stream_internal(endpoint_port, tcp_stream, configuration);

    let result = tokio::time::timeout(RESOLVE_TLS_TIMEOUT, future).await;

    if result.is_err() {
        return Err(format!(
            "Accepting TLS connection timeout for port: {}",
            endpoint_port
        ));
    }

    result.unwrap()
}

async fn lazy_accept_tcp_stream_internal(
    endpoint_port: u16,
    tcp_stream: tokio::net::TcpStream,
    configuration: Arc<HttpListenPortConfiguration>,
) -> Result<
    (
        my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        Arc<HttpEndpointInfo>,
        Option<Arc<ClientCertificateData>>,
    ),
    String,
> {
    let result: Result<
        Result<
            (
                my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
                Arc<HttpEndpointInfo>,
                Option<Arc<ClientCertificateData>>,
            ),
            String,
        >,
        tokio::task::JoinError,
    > = tokio::spawn(async move {
        let lazy_acceptor = LazyConfigAcceptor::new(Acceptor::default(), tcp_stream);

        tokio::pin!(lazy_acceptor);

        let (tls_stream, endpoint_info, client_certificate) = match lazy_acceptor.as_mut().await {
            Ok(start_handshake) => {
                let client_hello = start_handshake.client_hello();
                let server_name = if let Some(server_name) = client_hello.server_name() {
                    server_name
                } else {
                    return Err("Unknown server name detecting from client hello".to_string());
                };

                if let Some(client_cert) = client_hello.client_cert_types() {
                    for client_cert in client_cert {
                        println!("Client_CERT: {:?}", client_cert.as_str());
                    }
                }

                if let Some(ca) = client_hello.certificate_authorities() {
                    for cn in ca {
                        println!("DistName: {:?}", cn);
                    }
                }

                let config_result =
                    super::tls_acceptor::create_config(configuration, server_name, endpoint_port)
                        .await;

                if let Err(err) = &config_result {
                    return Err(format!("Failed to create tls config. Err: {err:#}"));
                }

                let (config, endpoint_info, client_cert_cell) = config_result.unwrap();

                //println!("Created config");

                let tls_stream = start_handshake.into_stream(config.into()).await;

                if let Err(err) = &tls_stream {
                    return Err(format!("failed to perform tls handshake: {err:#}"));
                }

                let tls_stream = tls_stream.unwrap();

                //println!("Applied config");
                let client_certificate = if let Some(client_cert_cell) = client_cert_cell {
                    client_cert_cell.get()
                } else {
                    None
                };

                (tls_stream, endpoint_info, client_certificate)
            }
            Err(err) => {
                return Err(format!("failed to perform tls handshake: {err:#}"));
            }
        };

        Ok((tls_stream, endpoint_info, client_certificate))
    })
    .await;

    if let Err(err) = result {
        return Err(format!("failed to perform tls handshake: {err:#}"));
    }

    let result = result.unwrap();

    result
}
