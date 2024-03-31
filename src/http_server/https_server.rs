use std::{net::SocketAddr, sync::Arc};

use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio_rustls::rustls::sign::CertifiedKey;

use tokio_rustls::{rustls::server::Acceptor, LazyConfigAcceptor};

use crate::app::{AppContext, SslCertificate};

use crate::http_proxy_pass::{HttpProxyPass, HttpServerConnectionInfo};

pub fn start_https_server(addr: SocketAddr, app: Arc<AppContext>, certificate: SslCertificate) {
    println!("Listening https://{}", addr);

    tokio::spawn(start_https_server_loop(addr, app, certificate));
}

async fn start_https_server_loop(
    addr: SocketAddr,
    app: Arc<AppContext>,
    certificate: SslCertificate,
) {
    let endpoint_port = addr.port();
    //let endpoint_info = Arc::new(endpoint_info);
    let certified_key = Arc::new(certificate.get_certified_key());
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    // Build TLS configuration.

    let mut connection_id = 0;

    loop {
        connection_id += 1;

        let (tcp_stream, socket_addr) = listener.accept().await.unwrap();

        println!("Accepted connection");

        let result = lazy_accept_tcp_stream(
            app.clone(),
            endpoint_port,
            connection_id,
            certified_key.clone(),
            tcp_stream,
        )
        .await;

        if let Err(err) = &result {
            eprintln!("failed to perform tls handshake: {err:#}");
            continue;
        }

        let (tls_stream, endpoint_info, cn_user_name) = result.unwrap();

        if endpoint_info.http_type.is_http1() {
            kick_off_https1(
                app.clone(),
                socket_addr,
                endpoint_info,
                tls_stream,
                cn_user_name,
            );
        } else {
            kick_off_https2(
                app.clone(),
                socket_addr,
                endpoint_info,
                tls_stream,
                cn_user_name,
            );
        }
    }
}

async fn lazy_accept_tcp_stream(
    app: Arc<AppContext>,
    endpoint_port: u16,
    connection_id: u64,
    certified_key: Arc<CertifiedKey>,
    tcp_stream: TcpStream,
) -> Result<
    (
        tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        HttpServerConnectionInfo,
        Option<String>,
    ),
    String,
> {
    let result = tokio::spawn(async move {
        let lazy_acceptor = LazyConfigAcceptor::new(Acceptor::default(), tcp_stream);
        tokio::pin!(lazy_acceptor);
        let (tls_stream, endpoint_info, cn_user_name) = match lazy_acceptor.as_mut().await {
            Ok(start) => {
                let client_hello = start.client_hello();
                let server_name = if let Some(server_name) = client_hello.server_name() {
                    server_name
                } else {
                    return Err("Unknown server name detecting from client hello".to_string());
                };

                let config_result = super::tls_acceptor::create_config(
                    app.clone(),
                    server_name,
                    endpoint_port,
                    connection_id,
                    certified_key,
                )
                .await;

                if let Err(err) = &config_result {
                    return Err(format!("failed to create tls config: {err:#}"));
                }

                let (config, endpoint_info) = config_result.unwrap();

                let tls_stream = start.into_stream(config.into()).await.unwrap();

                let cn_user_name = if endpoint_info.client_certificate_id.is_some() {
                    app.saved_client_certs.get(endpoint_port, connection_id)
                } else {
                    None
                };
                println!("Cert common name: {:?}", cn_user_name);
                (tls_stream, endpoint_info, cn_user_name)
            }
            Err(err) => {
                return Err(format!("failed to perform tls handshake: {err:#}"));
            }
        };

        Ok((tls_stream, endpoint_info, cn_user_name))
    })
    .await;

    if let Err(err) = result {
        return Err(format!("failed to perform tls handshake: {err:#}"));
    }

    let result = result.unwrap();

    result
}

fn kick_off_https1(
    app: Arc<AppContext>,
    socket_addr: SocketAddr,
    endpoint_info: HttpServerConnectionInfo,
    tls_stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    cn_user_name: Option<String>,
) {
    use hyper::{server::conn::http1, service::service_fn};
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    tokio::spawn(async move {
        let modify_headers_settings = app
            .settings_reader
            .get_http_endpoint_modify_headers_settings(&endpoint_info)
            .await;

        let http_proxy_pass = Arc::new(HttpProxyPass::new(
            socket_addr,
            modify_headers_settings,
            endpoint_info.into(),
            cn_user_name,
        ));

        /*
                   let (tls_stream, client_cert_cn) = match tls_acceptor.accept(tcp_stream).await {
                       Ok(tls_stream) => {
                           let cert_common_name = if has_client_cert_ca {
                               app.saved_client_certs.get(endpoint_port, connection_id)
                           } else {
                               None
                           };
                           println!("Cert common name: {:?}", cert_common_name);
                           (tls_stream, cert_common_name)
                       }
                       Err(err) => {
                           if has_client_cert_ca {
                               app.saved_client_certs.get(endpoint_port, connection_id);
                           }
                           eprintln!("failed to perform tls handshake: {err:#}");
                           return;
                       }
                   };
        */

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

fn kick_off_https2(
    app: Arc<AppContext>,
    socket_addr: SocketAddr,
    endpoint_info: HttpServerConnectionInfo,
    tls_stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    cn_user_name: Option<String>,
) {
    use hyper::service::service_fn;
    use hyper_util::server::conn::auto::Builder;

    use hyper_util::rt::TokioExecutor;

    tokio::spawn(async move {
        let http_builder = Builder::new(TokioExecutor::new());

        let modify_headers_settings = app
            .settings_reader
            .get_http_endpoint_modify_headers_settings(&endpoint_info)
            .await;

        let http_proxy_pass = Arc::new(HttpProxyPass::new(
            socket_addr,
            modify_headers_settings,
            endpoint_info.into(),
            cn_user_name,
        ));

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

/*
fn create_tls_acceptor(
    app: Arc<AppContext>,
    client_cert_ca: Option<Arc<ClientCertificateCa>>,
    endpoint_port: u16,
    connection_id: u64,
    certified_key: Arc<CertifiedKey>,
) -> TlsAcceptor {
    if let Some(client_cert_ca) = client_cert_ca {
        let client_cert_verifier = Arc::new(MyClientCertVerifier::new(
            app,
            client_cert_ca,
            endpoint_port,
            connection_id,
        ));

        let mut server_config =
            tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
                .with_client_cert_verifier(client_cert_verifier)
                .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

        server_config.alpn_protocols = vec![b"http/1.1".to_vec()];

        return TlsAcceptor::from(Arc::new(server_config));
    }

    let mut server_config =
        tokio_rustls::rustls::ServerConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(MyCertResolver::new(certified_key)));

    server_config.alpn_protocols = vec![b"http/1.1".to_vec()];

    // server_config.key_log = Arc::new(MyKeyLog);

    TlsAcceptor::from(Arc::new(server_config))
}
 */
