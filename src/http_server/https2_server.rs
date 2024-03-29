use std::{net::SocketAddr, sync::Arc};

use hyper::service::service_fn;

use tokio_rustls::rustls::sign::CertifiedKey;
use tokio_rustls::rustls::version::{TLS12, TLS13};
use tokio_rustls::TlsAcceptor;

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;

use crate::app::{AppContext, SslCertificate};

use super::server_cert_resolver::MyCertResolver;
use super::{ClientCertificateCa, MyClientCertVerifier};

use crate::http_proxy_pass::{HttpProxyPass, ProxyPassEndpointInfo};

pub fn start_https2_server(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
    client_cert_ca: Option<ClientCertificateCa>,
    endpoint_info: ProxyPassEndpointInfo,
) {
    println!("Listening h2 on https://{}", addr);

    let client_cert_ca = if let Some(client_cert_ca) = client_cert_ca {
        Some(Arc::new(client_cert_ca))
    } else {
        None
    };

    tokio::spawn(start_https2_server_loop(
        addr,
        app,
        certificate,
        client_cert_ca,
        endpoint_info,
    ));
}

async fn start_https2_server_loop(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
    client_cert_ca: Option<Arc<ClientCertificateCa>>,

    endpoint_info: ProxyPassEndpointInfo,
) {
    let endpoint_port = addr.port();
    let endpoint_info = Arc::new(endpoint_info);
    let certified_key = Arc::new(certificate.get_certified_key());
    //let certified_key = certificate.get_certified_key();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    let has_client_cert_ca = client_cert_ca.is_some();

    // Build TLS configuration.
    let http_builder = Builder::new(TokioExecutor::new());

    let mut connection_id = 0;

    loop {
        connection_id += 1;

        let (tcp_stream, socket_addr) = listener.accept().await.unwrap();

        let tls_acceptor = create_tls_acceptor(
            app.clone(),
            client_cert_ca.clone(),
            endpoint_port,
            connection_id,
            certified_key.clone(),
        );

        let app = app.clone();

        let modify_headers_settings = app
            .settings_reader
            .get_http_endpoint_modify_headers_settings(&endpoint_info)
            .await;

        let http_builder = http_builder.clone();

        let endpoint_info = endpoint_info.clone();

        tokio::spawn(async move {
            let http_proxy_pass = Arc::new(HttpProxyPass::new(
                socket_addr,
                modify_headers_settings,
                endpoint_info.clone(),
            ));

            let (tls_stream, client_cert_cn) = match tls_acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => {
                    let cert_common_name = if has_client_cert_ca {
                        app.saved_client_certs.get(endpoint_port, connection_id)
                    } else {
                        None
                    };

                    (tls_stream, cert_common_name)
                }
                Err(err) => {
                    if has_client_cert_ca {
                        app.saved_client_certs.get(endpoint_port, connection_id);
                    }
                    if endpoint_info.debug {
                        eprintln!("failed to perform tls handshake: {err:#}");
                    }
                    return;
                }
            };

            if let Some(client_cert_cn) = client_cert_cn {
                http_proxy_pass
                    .update_client_cert_cn_name(client_cert_cn)
                    .await;
            }

            if let Err(err) = http_builder
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
                .await
            {
                eprintln!("failed to serve connection: {err:#}");
            }
        });
    }
}

fn create_tls_acceptor(
    app: Arc<AppContext>,
    client_cert_ca: Option<Arc<ClientCertificateCa>>,
    endpoint_port: u16,
    connection_id: u64,
    certified_key: Arc<CertifiedKey>,
) -> TlsAcceptor {
    if let Some(client_cert_ca) = client_cert_ca {
        let client_cert_verifier = Arc::new(MyClientCertVerifier::new(
            app.clone(),
            client_cert_ca,
            endpoint_port,
            connection_id,
        ));

        let mut server_config =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
                .with_client_cert_verifier(client_cert_verifier)
                .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

        server_config.alpn_protocols =
            vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];

        return TlsAcceptor::from(Arc::new(server_config));
    }

    let mut server_config =
        tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];

    TlsAcceptor::from(Arc::new(server_config))
}
