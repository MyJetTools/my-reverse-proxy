use std::{net::SocketAddr, sync::Arc};

use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http2, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio_rustls::TlsAcceptor;

use crate::{
    app::{AppContext, SslCertificate},
    http_server::ProxyPassError,
};

use super::{ClientCertificateCa, MyClientCertVerifier, ProxyPassClient};

pub fn start_https_server(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
    client_cert_ca: Option<ClientCertificateCa>,
    server_id: i64,
) {
    println!("Listening https on https://{}", addr);
    tokio::spawn(start_https_server_loop(
        addr,
        app,
        certificate,
        client_cert_ca,
        server_id,
    ));
}

async fn start_https_server_loop(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
    client_cert_ca: Option<ClientCertificateCa>,
    server_id: i64,
) {
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    let has_client_cert_ca = client_cert_ca.is_some();

    let tls_acceptor = if let Some(client_cert_ca) = client_cert_ca {
        let client_cert_verifier = Arc::new(MyClientCertVerifier::new(
            app.clone(),
            client_cert_ca,
            server_id,
        ));
        let mut server_config = tokio_rustls::rustls::ServerConfig::builder()
            .with_client_cert_verifier(client_cert_verifier)
            .with_single_cert(
                certificate.certificates,
                certificate.private_key.clone_key(),
            )
            .unwrap();

        server_config.alpn_protocols =
            vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];

        TlsAcceptor::from(Arc::new(server_config))
    } else {
        let mut server_config = tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                certificate.certificates,
                certificate.private_key.clone_key(),
            )
            .unwrap();

        server_config.alpn_protocols =
            vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];

        TlsAcceptor::from(Arc::new(server_config))
    };

    // Build TLS configuration.

    loop {
        if has_client_cert_ca {
            println!("Waiting until we get common_name");
            app.saved_client_certs.wait_while_we_read_it(server_id);
            println!("Waited until we get common_name");
        }

        let (tcp_stream, socket_addr) = listener.accept().await.unwrap();

        let tls_acceptor = tls_acceptor.clone();

        let app = app.clone();

        tokio::spawn(async move {
            let http_proxy_pass = Arc::new(ProxyPassClient::new(socket_addr));

            let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => {
                    if has_client_cert_ca {
                        let common_name = app.saved_client_certs.get(server_id);
                        println!(
                            "Accepted TLS connection from: {:#?} with common name: {:#?}",
                            socket_addr, common_name
                        );
                    }
                    tls_stream
                }
                Err(err) => {
                    if has_client_cert_ca {
                        app.saved_client_certs.get(server_id);
                    }
                    eprintln!("failed to perform tls handshake: {err:#}");
                    return;
                }
            };

            if let Err(err) = http2::Builder::new(TokioExecutor::new())
                .serve_connection(
                    TokioIo::new(tls_stream),
                    service_fn(move |req| {
                        handle_requests(req, http_proxy_pass.clone(), app.clone())
                    }),
                )
                .await
            {
                eprintln!("failed to serve connection: {err:#}");
            }
        });
    }
}

pub async fn handle_requests(
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: Arc<ProxyPassClient>,
    app: Arc<AppContext>,
) -> hyper::Result<hyper::Response<Full<Bytes>>> {
    //println!("Handling request with host: {:#?}", req);
    match proxy_pass.send_payload(&app, req).await {
        Ok(response) => return response,
        Err(err) => {
            if err.is_timeout() {
                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Full::from(Bytes::from("Timeout")))
                    .unwrap());
            }

            match err {
                ProxyPassError::NoLocationFound => {
                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::NOT_FOUND)
                        .body(Full::from(Bytes::from("Not Found")))
                        .unwrap());
                }
                _ => {
                    println!("Error: {:?}", err);

                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Full::from(Bytes::from("Internal Server Error")))
                        .unwrap());
                }
            }
        }
    }
}
