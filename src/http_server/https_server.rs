use std::time::Duration;
use std::{net::SocketAddr, sync::Arc};

use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use my_tls::tokio_rustls::{rustls::server::Acceptor, LazyConfigAcceptor};

use crate::app::AppContext;

use crate::configurations::*;
use crate::http_proxy_pass::HttpProxyPass;
use crate::http_server::handle_request::HttpRequestHandler;

use super::ClientCertificateData;

pub fn start_https_server(addr: SocketAddr, app: Arc<AppContext>, debug: bool) {
    println!("Listening https://{}", addr);

    tokio::spawn(start_https_server_loop(addr, app, debug));
}

async fn start_https_server_loop(addr: SocketAddr, app: Arc<AppContext>, debug: bool) {
    let endpoint_port = addr.port();
    //let endpoint_info = Arc::new(endpoint_info);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    let endpoint_name = format!("https://{}", addr);
    let endpoint_name = Arc::new(endpoint_name);

    // Build TLS configuration.

    loop {
        // println!("Waiting to accept new connection");

        let accepted_connection = listener.accept().await;

        if app.states.is_shutting_down() {
            println!("Shutting down https server");
            break;
        }

        if let Err(err) = &accepted_connection {
            println!("Error accepting connection {}. Err: {:?}", addr, err);
            continue;
        }

        let (tcp_stream, socket_addr) = accepted_connection.unwrap();

        if debug {
            println!("Accepted connection from  {}", socket_addr);
        }

        let app = app.clone();
        handle_connection(
            app,
            endpoint_name.clone(),
            endpoint_port,
            tcp_stream,
            socket_addr,
            debug,
        )
        .await;
    }
}

async fn handle_connection(
    app: Arc<AppContext>,
    endpoint_name: Arc<String>,
    endpoint_port: u16,
    tcp_stream: TcpStream,
    socket_addr: SocketAddr,
    debug: bool,
) {
    let future = lazy_accept_tcp_stream(app.clone(), endpoint_port, tcp_stream, debug);

    let result = tokio::time::timeout(Duration::from_secs(10), future).await;

    if result.is_err() {
        if debug {
            println!("Timeout waiting for tls handshake from {}", socket_addr);
        }

        return;
    }

    let result = result.unwrap();

    if let Err(err) = &result {
        if debug {
            println!("failed to perform tls handshake: {err:#}");
        }

        return;
    }

    let (tls_stream, endpoint_info, cn_user_name) = result.unwrap();

    if endpoint_info.http_type.is_protocol_http1() {
        kick_off_https1(
            app,
            endpoint_name.clone(),
            socket_addr,
            endpoint_info,
            tls_stream,
            cn_user_name,
            debug,
        )
        .await;
    } else {
        kick_off_https2(
            app,
            endpoint_name.clone(),
            socket_addr,
            endpoint_info,
            tls_stream,
            cn_user_name,
            debug,
        )
        .await;
    }
}

async fn lazy_accept_tcp_stream(
    app: Arc<AppContext>,
    endpoint_port: u16,
    tcp_stream: TcpStream,
    debug: bool,
) -> Result<
    (
        my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        Arc<HttpEndpointInfo>,
        Option<ClientCertificateData>,
    ),
    String,
> {
    let result = tokio::spawn(async move {
        let lazy_acceptor = LazyConfigAcceptor::new(Acceptor::default(), tcp_stream);

        tokio::pin!(lazy_acceptor);

        let (tls_stream, endpoint_info, client_certificate) = match lazy_acceptor.as_mut().await {
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
                    debug,
                )
                .await;

                if let Err(err) = &config_result {
                    return Err(format!("Failed to create tls config. Err: {err:#}"));
                }

                let (config, endpoint_info, client_cert_cell) = config_result.unwrap();

                //println!("Created config");

                let tls_stream = start.into_stream(config.into()).await;

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

                if let Some(client_certificate) = client_certificate.as_ref() {
                    let app_config = app.get_current_app_configuration().await;

                    let list_of_crl = app_config.list_of_crl.lock().await;
                    if list_of_crl.has_certificate_as_revoked(client_certificate) {
                        return Err(format!(
                            "Client certificate {:?} is revoked",
                            client_certificate
                        ));
                    }
                }

                if debug {
                    println!("Cert common name: {:?}", client_certificate);
                }

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

async fn kick_off_https1(
    app: Arc<AppContext>,
    endpoint_name: Arc<String>,
    socket_addr: SocketAddr,
    endpoint_info: Arc<HttpEndpointInfo>,
    tls_stream: my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    cn_user_name: Option<ClientCertificateData>,
    debug: bool,
) {
    use hyper::{server::conn::http1, service::service_fn};
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    app.prometheus
        .inc_http1_server_connections(endpoint_name.as_str());

    app.metrics
        .update(|itm| itm.connection_by_port.inc(&socket_addr.port()))
        .await;
    println!("New https connection from {}", socket_addr);
    tokio::spawn(async move {
        let listening_port_info = endpoint_info.get_listening_port_info(socket_addr);

        let http_proxy_pass =
            HttpProxyPass::new(&app, endpoint_info, listening_port_info, cn_user_name);

        let http_request_handler = HttpRequestHandler::new(http_proxy_pass, app.clone());

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_dispose = http_request_handler.clone();

        if let Err(err) = http1
            .clone()
            .serve_connection(
                TokioIo::new(tls_stream),
                service_fn(move |req| {
                    super::handle_request::handle_request(http_request_handler.clone(), req)
                }),
            )
            .with_upgrades()
            .await
        {
            if debug {
                println!("failed to serve HTTP 1.1 connection: {err:#}");
            }
        }

        app.prometheus
            .dec_http1_server_connections(endpoint_name.as_str());

        app.metrics
            .update(|itm| itm.connection_by_port.dec(&socket_addr.port()))
            .await;

        println!("Gone https connection from {}", socket_addr);

        http_request_handler_dispose.dispose().await;
    });
}

async fn kick_off_https2(
    app: Arc<AppContext>,
    endpoint_name: Arc<String>,
    socket_addr: SocketAddr,
    endpoint_info: Arc<HttpEndpointInfo>,
    tls_stream: my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    client_certificate: Option<ClientCertificateData>,
    debug: bool,
) {
    use hyper::service::service_fn;
    use hyper_util::server::conn::auto::Builder;

    use hyper_util::rt::TokioExecutor;

    app.prometheus
        .inc_http2_server_connections(endpoint_name.as_str());

    app.metrics
        .update(|itm| itm.connection_by_port.inc(&socket_addr.port()))
        .await;

    println!("New Https2 connection from {}", socket_addr);

    tokio::spawn(async move {
        let http_builder = Builder::new(TokioExecutor::new());

        let listening_port_info = endpoint_info.get_listening_port_info(socket_addr);

        let http_proxy_pass =
            HttpProxyPass::new(&app, endpoint_info, listening_port_info, client_certificate);

        let http_request_handler = HttpRequestHandler::new(http_proxy_pass, app.clone());

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_dispose = http_request_handler.clone();

        if let Err(err) = http_builder
            .clone()
            .serve_connection(
                TokioIo::new(tls_stream),
                service_fn(move |req| {
                    super::handle_request::handle_request(http_request_handler.clone(), req)
                }),
            )
            .await
        {
            if debug {
                println!("failed to serve Https2 connection: {err:#}");
            }
        }

        app.prometheus
            .dec_http2_server_connections(endpoint_name.as_str());

        app.metrics
            .update(|itm| itm.connection_by_port.dec(&socket_addr.port()))
            .await;

        println!("Http2 connection is gone {}", socket_addr);

        http_request_handler_dispose.dispose().await;
    });
}
