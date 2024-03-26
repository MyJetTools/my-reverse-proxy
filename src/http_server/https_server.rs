use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use tokio_rustls::rustls::version::{TLS12, TLS13};
use tokio_rustls::TlsAcceptor;

use crate::app::{AppContext, SslCertificate};

use super::{ClientCertificateCa, MyClientCertVerifier};

use crate::http_proxy_pass::HttpProxyPass;

pub fn start_https_server(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
    client_cert_ca: Option<ClientCertificateCa>,
    server_id: i64,
    host_str: String,
    debug: bool,
) {
    println!("Listening http1 on https://{}", addr);
    tokio::spawn(start_https_server_loop(
        addr,
        app,
        certificate,
        client_cert_ca,
        server_id,
        host_str,
        debug,
    ));
}

async fn start_https_server_loop(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
    client_cert_ca: Option<ClientCertificateCa>,
    server_id: i64,
    host_configuration: String,
    debug: bool,
) {
    let host_configuration = Arc::new(host_configuration);
    //let certified_key = certificate.get_certified_key();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    let has_client_cert_ca = client_cert_ca.is_some();

    let tls_acceptor = if let Some(client_cert_ca) = client_cert_ca {
        let client_cert_verifier = Arc::new(MyClientCertVerifier::new(
            app.clone(),
            client_cert_ca,
            server_id,
        ));

        let mut server_config =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
                .with_client_cert_verifier(client_cert_verifier)
                .with_single_cert(
                    certificate.certificates.clone(),
                    certificate.private_key.as_ref().clone_key(),
                )
                .unwrap();

        server_config.alpn_protocols = vec![b"http/1.1".to_vec()];

        TlsAcceptor::from(Arc::new(server_config))
    } else {
        let mut server_config =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
                .with_no_client_auth()
                .with_single_cert(
                    certificate.certificates.clone(),
                    certificate.private_key.as_ref().clone_key(),
                )
                .unwrap();

        server_config.alpn_protocols = vec![b"http/1.1".to_vec()];

        // server_config.key_log = Arc::new(MyKeyLog);

        TlsAcceptor::from(Arc::new(server_config))
    };

    // Build TLS configuration.
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    loop {
        if has_client_cert_ca {
            println!("Waiting until we get common_name");
            app.saved_client_certs.wait_while_we_read_it(server_id);
            println!("Waited until we get common_name");
        }

        let (tcp_stream, socket_addr) = listener.accept().await.unwrap();

        let tls_acceptor = tls_acceptor.clone();

        let app = app.clone();

        let modify_headers_settings = app
            .settings_reader
            .get_http_endpoint_modify_headers_settings(host_configuration.as_str())
            .await;

        let http1 = http1.clone();

        let host_configuration = host_configuration.clone();

        tokio::spawn(async move {
            let http_proxy_pass = Arc::new(HttpProxyPass::new(
                socket_addr,
                modify_headers_settings,
                true,
                debug,
                host_configuration,
            ));

            let (tls_stream, client_cert_cn) = match tls_acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => {
                    let cert_common_name = if has_client_cert_ca {
                        app.saved_client_certs.get(server_id)
                    } else {
                        None
                    };
                    (tls_stream, cert_common_name)
                }
                Err(err) => {
                    if has_client_cert_ca {
                        app.saved_client_certs.get(server_id);
                    }
                    eprintln!("failed to perform tls handshake: {err:#}");
                    return;
                }
            };

            if let Some(client_cert_cn) = client_cert_cn {
                http_proxy_pass
                    .update_client_cert_cn_name(client_cert_cn)
                    .await;
            }

            if let Err(err) = http1
                .clone()
                .serve_connection(
                    TokioIo::new(tls_stream),
                    service_fn(move |req| {
                        super::handle_request::handle_requests(
                            req,
                            http_proxy_pass.clone(),
                            app.clone(),
                        )
                    }),
                )
                .with_upgrades()
                .await
            {
                eprintln!("failed to serve connection: {err:#}");
            }
        });
    }
}

/*

pub async fn handle_requests(
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: Arc<HttpProxyPass>,
    app: Arc<AppContext>,
) -> hyper::Result<hyper::Response<Full<Bytes>>> {
    let debug = if proxy_pass.debug {
        let req_str = format!("[{}]{:?}", req.method(), req.uri());
        let mut sw = StopWatch::new();
        sw.start();
        println!("Req: {}", req_str);
        Some((req_str, sw))
    } else {
        None
    };

    match proxy_pass.send_payload(&app, req).await {
        Ok(response) => {
            match response.as_ref() {
                Ok(response) => {
                    if let Some((req_str, mut sw)) = debug {
                        sw.pause();
                        println!(
                            "Res: {}->{} {}",
                            req_str,
                            response.status(),
                            sw.duration_as_string()
                        );
                    }
                }
                Err(err) => {
                    if let Some((req_str, mut sw)) = debug {
                        sw.pause();
                        println!("Res: {}->{} {}", req_str, err, sw.duration_as_string());
                    }
                }
            }

            return response;
        }
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
                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Full::from(Bytes::from("Internal Server Error")))
                        .unwrap());
                }
            }
        }
    }
}
 */
