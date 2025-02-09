use std::{net::SocketAddr, sync::Arc};

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::{app::AppContext, configurations::HttpListenPortConfiguration};

use super::{http_request_handler::http::HttpRequestHandler, AcceptedTcpConnection};

pub async fn handle_connection(
    app: Arc<AppContext>,
    accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<HttpListenPortConfiguration>,
) {
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

        let http_request_handler =
            HttpRequestHandler::new(app.clone(), accepted_connection.addr, configuration);

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_to_dispose = http_request_handler.clone();
        let result = builder
            .serve_connection(
                io,
                service_fn(move |req| {
                    super::http_request_handler::http::handle_request(
                        http_request_handler.clone(),
                        req,
                    )
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
