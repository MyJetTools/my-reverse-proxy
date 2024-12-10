use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use crate::{app::AppContext, configurations::HttpListenPortConfiguration};

use super::{handle_request::HttpRequestHandler, AcceptedTcpConnection};

/*
pub fn start_http_server(
    addr: SocketAddr,
    app: Arc<AppContext>,
    debug: bool,
) -> Arc<ListenServerHandler> {
    println!("Listening http1 on http://{}", addr);

    let listen_server_handler = Arc::new(ListenServerHandler::new());
    tokio::spawn(start_http_server_loop(
        addr,
        app,
        debug,
        listen_server_handler.clone(),
    ));

    listen_server_handler
}

async fn start_http_server_loop(
    listening_addr: SocketAddr,
    app: Arc<AppContext>,
    debug: bool,
    listen_server_handler: Arc<ListenServerHandler>,
) {
    let listener = tokio::net::TcpListener::bind(listening_addr).await.unwrap();
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);
    let listening_addr_str = format!("http://{}", listening_addr);
    let listening_addr_str = Arc::new(listening_addr_str);
    loop {
        let accepted_connection_feature = listener.accept();

        let stop_endpoint_feature = listen_server_handler.await_stop();

        tokio::select! {
            accepted_connection = accepted_connection_feature => {
                if let Err(err) = &accepted_connection {
                    if debug {
                        println!(
                            "Error accepting connection {}. Err: {:?}",
                            listening_addr, err
                        );
                    }
                    continue;
                }

                let (accepted_connection, accepted_socket_addr) = accepted_connection.unwrap();
                if debug {
                    println!("New connection accepted");
                }

                if app.states.is_shutting_down() {
                    println!("Shutting down http server");
                    return;
                }

                let io = TokioIo::new(accepted_connection);

                let http_request_handler =
                    HttpRequestHandler::new_lazy(app.clone(), listening_addr.port(), accepted_socket_addr);

                let http_request_handler = Arc::new(http_request_handler);

                let http_request_handler_disposed = http_request_handler.clone();

                let connection = http1
                    .serve_connection(
                        io,
                        service_fn(move |req| {
                            super::handle_request::handle_request(http_request_handler.clone(), req)
                        }),
                    )
                    .with_upgrades();

                let app = app.clone();

                app.prometheus
                    .inc_http1_server_connections(listening_addr_str.as_str());

                app.metrics
                    .update(|itm| itm.connection_by_port.inc(&listening_addr.port()))
                    .await;

                let listening_addr_str = listening_addr_str.clone();

                tokio::task::spawn(async move {
                    if let Err(err) = connection.await {
                        if debug {
                            println!(
                                "{}. Error serving connection: {:?}",
                                rust_extensions::date_time::DateTimeAsMicroseconds::now().to_rfc3339(),
                                err
                            );
                        }
                    }

                    app.prometheus
                        .dec_http1_server_connections(listening_addr_str.as_str());

                    app.metrics
                        .update(|itm| itm.connection_by_port.dec(&listening_addr.port()))
                        .await;
                    http_request_handler_disposed.dispose().await;
                });
            }
            _ = stop_endpoint_feature => {
                break;
            }
        }

        if !listen_server_handler.is_running() {
            println!("Http Endpoint {} is stopped", listening_addr);
            break;
        }
    }
}
 */
pub async fn handle_connection(
    app: Arc<AppContext>,
    accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    _configuration: Arc<HttpListenPortConfiguration>,
) {
    //todo!("Somehow we do not use configuration")
    let listening_addr_str = format!("http://{}", listening_addr);

    let io = TokioIo::new(accepted_connection.tcp_stream);

    let http_request_handler =
        HttpRequestHandler::new_lazy(app.clone(), listening_addr.port(), accepted_connection.addr);

    let http_request_handler = Arc::new(http_request_handler);

    let http_request_handler_disposed = http_request_handler.clone();

    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    let connection = http1
        .serve_connection(
            io,
            service_fn(move |req| {
                super::handle_request::handle_request(http_request_handler.clone(), req)
            }),
        )
        .with_upgrades();

    let app = app.clone();

    app.prometheus
        .inc_http1_server_connections(listening_addr_str.as_str());

    app.metrics
        .update(|itm| itm.connection_by_port.inc(&listening_addr.port()))
        .await;

    let listening_addr_str = listening_addr_str.clone();

    tokio::task::spawn(async move {
        if let Err(err) = connection.await {
            println!(
                "{}. Error serving connection: {:?}",
                rust_extensions::date_time::DateTimeAsMicroseconds::now().to_rfc3339(),
                err
            );
        }

        app.prometheus
            .dec_http1_server_connections(listening_addr_str.as_str());

        app.metrics
            .update(|itm| itm.connection_by_port.dec(&listening_addr.port()))
            .await;
        http_request_handler_disposed.dispose().await;
    });
}
