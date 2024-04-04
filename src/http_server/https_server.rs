use std::{net::SocketAddr, sync::Arc};

use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use tokio_rustls::{rustls::server::Acceptor, LazyConfigAcceptor};

use crate::app::AppContext;

use crate::app_configuration::HttpEndpointInfo;
use crate::http_proxy_pass::HttpProxyPass;
use crate::http_server::handle_request::HttpRequestHandler;

pub fn start_https_server(addr: SocketAddr, app: Arc<AppContext>) {
    println!("Listening https://{}", addr);

    tokio::spawn(start_https_server_loop(addr, app));
}

async fn start_https_server_loop(addr: SocketAddr, app: Arc<AppContext>) {
    let endpoint_port = addr.port();
    //let endpoint_info = Arc::new(endpoint_info);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    // Build TLS configuration.

    loop {
        let (tcp_stream, socket_addr) = listener.accept().await.unwrap();

        println!("Accepted connection");

        let result = lazy_accept_tcp_stream(app.clone(), endpoint_port, tcp_stream).await;

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
                endpoint_port,
            );
        } else {
            kick_off_https2(
                app.clone(),
                socket_addr,
                endpoint_info,
                tls_stream,
                cn_user_name,
                endpoint_port,
            );
        }
    }
}

async fn lazy_accept_tcp_stream(
    app: Arc<AppContext>,
    endpoint_port: u16,
    tcp_stream: TcpStream,
) -> Result<
    (
        tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        Arc<HttpEndpointInfo>,
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

                let config_result =
                    super::tls_acceptor::create_config(app.clone(), server_name, endpoint_port)
                        .await;

                if let Err(err) = &config_result {
                    return Err(format!("Failed to create tls config. Err: {err:#}"));
                }

                let (config, endpoint_info, client_cert_cell) = config_result.unwrap();

                println!("Created config");

                let tls_stream = start.into_stream(config.into()).await.unwrap();

                println!("Applied config");
                let cn_user_name = if let Some(client_cert_cell) = client_cert_cell {
                    client_cert_cell.get()
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
    endpoint_info: Arc<HttpEndpointInfo>,
    tls_stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    cn_user_name: Option<String>,
    listening_port: u16,
) {
    use hyper::{server::conn::http1, service::service_fn};
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    tokio::spawn(async move {
        let listening_port_info =
            endpoint_info.get_listening_port_info(listening_port, socket_addr);

        let http_proxy_pass = HttpProxyPass::new(
            endpoint_info,
            listening_port_info,
            cn_user_name,
            app.connection_settings.remote_connect_timeout,
        );

        let http_request_handler = HttpRequestHandler::new(http_proxy_pass, app.clone());

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_dispose = http_request_handler.clone();

        if let Err(err) = http1
            .clone()
            .serve_connection(
                TokioIo::new(tls_stream),
                service_fn(move |req| {
                    super::handle_request::handle_request(
                        http_request_handler.clone(),
                        req,
                        app.connection_settings.remote_connect_timeout,
                    )
                }),
            )
            .with_upgrades()
            .await
        {
            eprintln!("failed to serve connection: {err:#}");
        }

        http_request_handler_dispose.dispose().await;
    });
}

fn kick_off_https2(
    app: Arc<AppContext>,
    socket_addr: SocketAddr,
    endpoint_info: Arc<HttpEndpointInfo>,
    tls_stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    cn_user_name: Option<String>,
    listening_port: u16,
) {
    use hyper::service::service_fn;
    use hyper_util::server::conn::auto::Builder;

    use hyper_util::rt::TokioExecutor;

    tokio::spawn(async move {
        let http_builder = Builder::new(TokioExecutor::new());

        let listening_port_info =
            endpoint_info.get_listening_port_info(listening_port, socket_addr);

        let http_proxy_pass = HttpProxyPass::new(
            endpoint_info,
            listening_port_info,
            cn_user_name,
            app.connection_settings.remote_connect_timeout,
        );

        let http_request_handler = HttpRequestHandler::new(http_proxy_pass, app.clone());

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_dispose = http_request_handler.clone();

        if let Err(err) = http_builder
            .clone()
            .serve_connection(
                TokioIo::new(tls_stream),
                service_fn(move |req| {
                    super::handle_request::handle_request(
                        http_request_handler.clone(),
                        req,
                        app.connection_settings.remote_connect_timeout,
                    )
                }),
            )
            .await
        {
            eprintln!("failed to serve connection: {err:#}");
        }

        http_request_handler_dispose.dispose().await;
    });
}
