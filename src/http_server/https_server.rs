use std::{net::SocketAddr, sync::Arc};

use http_body_util::Full;
use hyper::{
    body::Bytes,
    server::conn::http1::{self, Builder},
    service::service_fn,
};
use hyper_util::rt::TokioIo;

use rustls::Certificate;
use tokio_rustls::TlsAcceptor;

use crate::{app::AppContext, http_server::ProxyPassError};

use super::ProxyPassClient;

pub fn start_https_server(addr: SocketAddr, app: Arc<AppContext>) {
    println!("Listening https1 on https://{}", addr);
    tokio::spawn(start_https_server_loop(addr, app));
}

async fn start_https_server_loop(addr: SocketAddr, app: Arc<AppContext>) {
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    // Load public certificate.
    let certs = load_certs().unwrap();
    // Load private key.
    let key = load_private_key().unwrap();

    // Build TLS configuration.
    let mut server_config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
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
            if let Err(err) = Builder::new()
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

        /*
        let io = TokioIo::new(stream);

        app.http_connections
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);



        let http_proxy_pass_to_dispose = http_proxy_pass.clone();

        let app = app.clone();

        let app_disposed = app.clone();

        let connection = http1
            .serve_connection(
                io,
                service_fn(move |req| handle_requests(req, http_proxy_pass.clone(), app.clone())),
            )
            .with_upgrades();

        tokio::task::spawn(async move {
            if let Err(err) = connection.await {
                println!(
                    "{}. Error serving connection: {:?}",
                    DateTimeAsMicroseconds::now().to_rfc3339(),
                    err
                );
            }

            http_proxy_pass_to_dispose.dispose().await;

            app_disposed
                .http_connections
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        });
         */
    }
}

pub async fn handle_requests(
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: Arc<ProxyPassClient>,
    app: Arc<AppContext>,
) -> hyper::Result<hyper::Response<Full<Bytes>>> {
    //println!("Handling request with host: {:?}", req.uri().host());
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

fn load_certs() -> std::io::Result<Vec<Certificate>> {
    let file_name = "/Users/amigin/certs/cert.cer";

    // Open certificate file.
    let certfile = std::fs::File::open(file_name)?;

    let mut reader = std::io::BufReader::new(certfile);

    let certs = rustls_pemfile::certs(&mut reader);

    // Load and return certificate.
    let mut result = Vec::new();

    for cert in certs {
        let cert: rustls_pki_types::CertificateDer<'_> = cert.unwrap();

        let cert = cert.as_ref();
        let cert = rustls::Certificate(cert.to_vec());
        result.push(cert);
    }

    Ok(result)
}

// Load private key from file.
fn load_private_key() -> std::io::Result<rustls::PrivateKey> {
    let file_name = "/Users/amigin/certs/cert.key";

    // Open keyfile.
    let keyfile = std::fs::File::open(file_name)?;
    let mut reader = std::io::BufReader::new(keyfile);

    let private_key = rustls_pemfile::private_key(&mut reader).unwrap();

    if private_key.is_none() {
        panic!("No private key found in file {}", file_name);
    }

    let private_key: rustls_pki_types::PrivateKeyDer<'_> = private_key.unwrap();

    let private_key = private_key.secret_der();

    Ok(rustls::PrivateKey(private_key.to_vec()))

    //  Ok(private_key.into())
}
