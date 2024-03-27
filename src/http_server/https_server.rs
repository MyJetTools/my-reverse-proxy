use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use tokio_rustls::rustls::version::{TLS12, TLS13};
use tokio_rustls::TlsAcceptor;

use crate::app::{AppContext, SslCertificate};

use super::{ClientCertificateCa, MyClientCertVerifier};

use crate::http_proxy_pass::{HttpProxyPass, ProxyPassEndpointInfo};

pub fn start_https_server(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
    client_cert_ca: Option<ClientCertificateCa>,
    server_id: i64,
    endpoint_info: ProxyPassEndpointInfo,
) {
    println!("Listening http1 on https://{}", addr);
    tokio::spawn(start_https_server_loop(
        addr,
        app,
        certificate,
        client_cert_ca,
        server_id,
        endpoint_info,
    ));
}

async fn start_https_server_loop(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
    client_cert_ca: Option<ClientCertificateCa>,
    server_id: i64,
    endpoint_info: ProxyPassEndpointInfo,
) {
    let endpoint_info = Arc::new(endpoint_info);
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
            .get_http_endpoint_modify_headers_settings(endpoint_info.as_ref())
            .await;

        let http1 = http1.clone();

        let endpoint_info = endpoint_info.clone();

        tokio::spawn(async move {
            let http_proxy_pass = Arc::new(HttpProxyPass::new(
                socket_addr,
                modify_headers_settings,
                endpoint_info,
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
