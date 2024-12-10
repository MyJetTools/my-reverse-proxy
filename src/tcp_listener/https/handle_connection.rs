use std::time::Duration;
use std::{net::SocketAddr, sync::Arc};

use hyper_util::rt::TokioIo;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use my_tls::tokio_rustls::{rustls::server::Acceptor, LazyConfigAcceptor};

use crate::app::AppContext;

use crate::configurations::*;
use crate::http_proxy_pass::{HttpListenPortInfo, HttpProxyPass};
use crate::tcp_listener::handle_request::HttpRequestHandler;
use crate::tcp_listener::AcceptedTcpConnection;

use super::ClientCertificateData;

/*
pub fn start_https_server(
    addr: SocketAddr,
    app: Arc<AppContext>,
    debug: bool,
) -> Arc<ListenServerHandler> {
    println!("Listening https://{}", addr);

    let listen_server_handler = Arc::new(ListenServerHandler::new());
    tokio::spawn(start_https_server_loop(
        addr,
        app,
        debug,
        listen_server_handler.clone(),
    ));
    listen_server_handler
}

async fn start_https_server_loop(
    addr: SocketAddr,
    app: Arc<AppContext>,
    debug: bool,
    listen_server_handler: Arc<ListenServerHandler>,
) {
    let endpoint_port = addr.port();
    //let endpoint_info = Arc::new(endpoint_info);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    let endpoint_name = format!("https://{}", addr);
    let endpoint_name = Arc::new(endpoint_name);

    // Build TLS configuration.

    loop {
        // println!("Waiting to accept new connection");

        let accepted_connection_future = listener.accept();

        let stop_endpoint_feature = listen_server_handler.await_stop();

        tokio::select! {
        _ = stop_endpoint_feature => {
           return
        }
        accepted_connection = accepted_connection_future => {
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

        if app.states.is_shutting_down() {
            println!("Shutting down https server");
            break;
        }
    }
}
 */

pub async fn handle_connection(
    app: Arc<AppContext>,
    accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<HttpListenPortConfiguration>,
) {
    let listening_addr_str = Arc::new(format!("https://{}", listening_addr));
    let endpoint_port = listening_addr.port();
    let future = lazy_accept_tcp_stream(
        app.clone(),
        endpoint_port,
        accepted_connection.tcp_stream,
        configuration,
    );

    let result = tokio::time::timeout(Duration::from_secs(10), future).await;

    if result.is_err() {
        return;
    }

    let result = result.unwrap();

    if result.is_err() {
        return;
    }

    let (mut tls_stream, endpoint_info, cn_user_name) = result.unwrap();

    if let Some(ip_list_id) = endpoint_info.whitelisted_ip_list_id.as_ref() {
        let is_whitelisted = app
            .current_configuration
            .get(|config| {
                config
                    .white_list_ip_list
                    .is_white_listed(ip_list_id, &listening_addr.ip())
            })
            .await;

        if !is_whitelisted {
            let _ = tls_stream.shutdown().await;
            return;
        }
    }

    if endpoint_info.listen_endpoint_type.is_http1() {
        kick_off_https1(
            app,
            listening_addr_str.clone(),
            accepted_connection.addr,
            endpoint_info,
            tls_stream,
            cn_user_name,
            endpoint_port,
        )
        .await;
    } else {
        kick_off_https2(
            app,
            listening_addr_str.clone(),
            accepted_connection.addr,
            endpoint_info,
            tls_stream,
            cn_user_name,
            endpoint_port,
        )
        .await;
    }
}

async fn lazy_accept_tcp_stream(
    app: Arc<AppContext>,
    endpoint_port: u16,
    tcp_stream: TcpStream,
    configuration: Arc<HttpListenPortConfiguration>,
) -> Result<
    (
        my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        Arc<HttpEndpointInfo>,
        Option<Arc<ClientCertificateData>>,
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
                    configuration,
                    server_name,
                    endpoint_port,
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
    cn_user_name: Option<Arc<ClientCertificateData>>,
    endpoint_port: u16,
) {
    use hyper::{server::conn::http1, service::service_fn};
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    app.prometheus
        .inc_http1_server_connections(endpoint_name.as_str());

    app.metrics
        .update(|itm| itm.connection_by_port.inc(&endpoint_port))
        .await;

    tokio::spawn(async move {
        let listening_port_info = HttpListenPortInfo {
            endpoint_type: endpoint_info.listen_endpoint_type,
            socket_addr,
        };
        let http_proxy_pass =
            HttpProxyPass::new(&app, endpoint_info, listening_port_info, cn_user_name).await;

        let http_request_handler =
            HttpRequestHandler::new(http_proxy_pass, app.clone(), socket_addr);

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_dispose = http_request_handler.clone();

        let _ = http1
            .clone()
            .serve_connection(
                TokioIo::new(tls_stream),
                service_fn(move |req| {
                    super::super::handle_request::handle_request(http_request_handler.clone(), req)
                }),
            )
            .with_upgrades()
            .await;

        app.prometheus
            .dec_http1_server_connections(endpoint_name.as_str());

        app.metrics
            .update(|itm| itm.connection_by_port.dec(&endpoint_port))
            .await;

        http_request_handler_dispose.dispose().await;
    });
}

async fn kick_off_https2(
    app: Arc<AppContext>,
    endpoint_name: Arc<String>,
    socket_addr: SocketAddr,
    endpoint_info: Arc<HttpEndpointInfo>,
    tls_stream: my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    client_certificate: Option<Arc<ClientCertificateData>>,
    endpoint_port: u16,
) {
    use hyper::service::service_fn;
    use hyper_util::server::conn::auto::Builder;

    use hyper_util::rt::TokioExecutor;

    app.prometheus
        .inc_http2_server_connections(endpoint_name.as_str());

    app.metrics
        .update(|itm| itm.connection_by_port.inc(&endpoint_port))
        .await;

    tokio::spawn(async move {
        let http_builder = Builder::new(TokioExecutor::new());

        let listening_port_info = HttpListenPortInfo {
            endpoint_type: endpoint_info.listen_endpoint_type,
            socket_addr,
        };

        let http_proxy_pass =
            HttpProxyPass::new(&app, endpoint_info, listening_port_info, client_certificate).await;

        let http_request_handler =
            HttpRequestHandler::new(http_proxy_pass, app.clone(), socket_addr);

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_dispose = http_request_handler.clone();

        let _ = http_builder
            .clone()
            .serve_connection(
                TokioIo::new(tls_stream),
                service_fn(move |req| {
                    super::super::handle_request::handle_request(http_request_handler.clone(), req)
                }),
            )
            .await;

        app.prometheus
            .dec_http2_server_connections(endpoint_name.as_str());

        app.metrics
            .update(|itm| itm.connection_by_port.dec(&endpoint_port))
            .await;

        println!("Http2 connection is gone {}", socket_addr);

        http_request_handler_dispose.dispose().await;
    });
}
