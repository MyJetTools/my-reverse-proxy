use std::{net::SocketAddr, sync::Arc};

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::{app::AppContext, configurations::HttpListenPortConfiguration};

use super::{handle_request::HttpRequestHandler, AcceptedTcpConnection};

/*
pub fn start_h2_server(
    addr: SocketAddr,
    app: Arc<AppContext>,
    debug: bool,
) -> Arc<ListenServerHandler> {
    println!("Listening h2 on http://{}", addr);

    let listen_server_handler = Arc::new(ListenServerHandler::new());
    tokio::spawn(start_https2_server_loop(
        addr,
        app,
        debug,
        listen_server_handler.clone(),
    ));

    listen_server_handler
}

async fn start_https2_server_loop(
    listening_addr: SocketAddr,
    app: Arc<AppContext>,
    debug: bool,
    listen_server_handler: Arc<ListenServerHandler>,
) {
    let listener = tokio::net::TcpListener::bind(listening_addr).await.unwrap();
    let http2_builder = Arc::new(hyper::server::conn::http2::Builder::new(
        TokioExecutor::new(),
    ));
    let listening_addr_str = format!("http://{}", listening_addr);
    let listening_addr_str = Arc::new(listening_addr_str);
    loop {
        let accepted_connection_future = listener.accept();

        let stop_endpoint_feature = listen_server_handler.await_stop();

        tokio::select! {
            _ = stop_endpoint_feature => {
                return;
            }
            accepted_connection = accepted_connection_future => {
                if app.states.is_shutting_down() {
                    println!("Shutting down h2 server");
                    break;
                }

                if let Err(err) = &accepted_connection {
                    if debug {
                        println!(
                            "Error accepting connection {}. Err: {:?}",
                            listening_addr, err
                        );
                    }
                    continue;
                }

                let (stream, socket_addr) = accepted_connection.unwrap();

                let app = app.clone();
                let builder = http2_builder.clone();

                app.prometheus
                    .inc_http1_server_connections(listening_addr_str.as_str());

                app.metrics
                    .update(|itm| itm.connection_by_port.inc(&listening_addr.port()))
                    .await;

                let listening_addr_str = listening_addr_str.clone();

                app.prometheus
                    .inc_http2_server_connections(listening_addr_str.as_str());
                tokio::spawn(async move {
                    let io = TokioIo::new(stream);

                    let http_request_handler =
                        HttpRequestHandler::new_lazy(app.clone(), listening_addr.port(), socket_addr);

                    let http_request_handler = Arc::new(http_request_handler);

                    let http_request_handler_to_dispose = http_request_handler.clone();
                    let _ = builder
                        .serve_connection(
                            io,
                            service_fn(move |req| {
                                super::handle_request::handle_request(http_request_handler.clone(), req)
                            }),
                        )
                        .await;

                    app.metrics
                        .update(|itm| itm.connection_by_port.dec(&listening_addr.port()))
                        .await;

                    app.prometheus
                        .dec_http2_server_connections(listening_addr_str.as_str());
                    http_request_handler_to_dispose.dispose().await;
                });
            }
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

    let http2_builder = Arc::new(hyper::server::conn::http2::Builder::new(
        TokioExecutor::new(),
    ));

    let builder = http2_builder.clone();

    app.prometheus
        .inc_http1_server_connections(listening_addr_str.as_str());

    app.metrics
        .update(|itm| itm.connection_by_port.inc(&listening_addr.port()))
        .await;

    let listening_addr_str = listening_addr_str.clone();

    app.prometheus
        .inc_http2_server_connections(listening_addr_str.as_str());
    tokio::spawn(async move {
        let io = TokioIo::new(accepted_connection.tcp_stream);

        let http_request_handler = HttpRequestHandler::new_lazy(
            app.clone(),
            listening_addr.port(),
            accepted_connection.addr,
        );

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_to_dispose = http_request_handler.clone();
        let result = builder
            .serve_connection(
                io,
                service_fn(move |req| {
                    super::handle_request::handle_request(http_request_handler.clone(), req)
                }),
            )
            .await;

        if let Err(err) = result {
            println!(
                "Error serving H2 connection on [{}]. Err{:?}",
                listening_addr_str, err
            );
        }

        app.metrics
            .update(|itm| itm.connection_by_port.dec(&listening_addr.port()))
            .await;

        app.prometheus
            .dec_http2_server_connections(listening_addr_str.as_str());
        http_request_handler_to_dispose.dispose().await;
    });
}
