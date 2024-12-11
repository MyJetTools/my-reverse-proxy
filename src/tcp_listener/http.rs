use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use crate::{app::AppContext, configurations::HttpListenPortConfiguration};

use super::{http_request_handler::http::HttpRequestHandler, AcceptedTcpConnection};

pub async fn handle_connection(
    app: Arc<AppContext>,
    accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<HttpListenPortConfiguration>,
) {
    let listening_addr_str = format!("http://{}", listening_addr);

    let io = TokioIo::new(accepted_connection.tcp_stream);

    let http_request_handler =
        HttpRequestHandler::new(app.clone(), accepted_connection.addr, configuration);

    let http_request_handler = Arc::new(http_request_handler);

    let http_request_handler_disposed = http_request_handler.clone();

    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    let connection = http1
        .serve_connection(
            io,
            service_fn(move |req| {
                super::http_request_handler::http::handle_request(http_request_handler.clone(), req)
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
