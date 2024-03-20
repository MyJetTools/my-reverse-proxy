use std::{net::SocketAddr, sync::Arc};

use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http2, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};

use tokio_rustls::TlsAcceptor;

use crate::{
    app::{AppContext, SslCertificate},
    http_server::ProxyPassError,
};

use super::ProxyPassClient;

pub fn start_https_server(addr: SocketAddr, app: Arc<AppContext>, certificate: SslCertificate) {
    println!("Listening https on https://{}", addr);
    tokio::spawn(start_https_server_loop(addr, app, certificate));
}

async fn start_https_server_loop(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
) {
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    // Build TLS configuration.
    let mut server_config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certificate.certificates, certificate.private_key)
        .unwrap();

    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];
    let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));

    loop {
        let (tcp_stream, socket_addr) = listener.accept().await.unwrap();

        let tls_acceptor = tls_acceptor.clone();

        let app = app.clone();

        let http_proxy_pass = Arc::new(ProxyPassClient::new(socket_addr));

        tokio::spawn(async move {
            let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => tls_stream,
                Err(err) => {
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
    //println!("Handling request with host: {:?}", req);
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
