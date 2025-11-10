use std::sync::Arc;

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::{
    configurations::HttpListenPortConfiguration,
    network_stream::*,
    types::{AcceptedServerConnection, ListenHost},
};

use super::http_request_handler::http::HttpRequestHandler;

pub async fn handle_connection(
    accepted_connection: AcceptedServerConnection,
    listen_host: ListenHost,
    configuration: Arc<HttpListenPortConfiguration>,
) {
    let listening_addr_str =
        listen_host.to_pretty_string(configuration.listen_endpoint_type.is_https());

    let http2_builder = Arc::new(hyper::server::conn::http2::Builder::new(
        TokioExecutor::new(),
    ));

    let builder = http2_builder.clone();

    crate::app::APP_CTX
        .prometheus
        .inc_http1_server_connections(listening_addr_str.as_str());

    let port = listen_host.get_port();

    if let Some(port) = port.as_ref() {
        crate::app::APP_CTX
            .metrics
            .update(|itm| itm.connection_by_port.inc(port))
            .await;
    }

    let listening_addr_str = listening_addr_str.clone();

    crate::app::APP_CTX
        .prometheus
        .inc_http2_server_connections(listening_addr_str.as_str());

    let connection_addr = accepted_connection.get_addr();
    tokio::spawn(async move {
        let io = match accepted_connection {
            AcceptedServerConnection::Tcp { network_stream, .. } => {
                let io = TokioIo::new(network_stream);

                TcpOrUnixSocket::Tcp(io)
            }
            AcceptedServerConnection::Unix(unix_stream) => {
                let io = TokioIo::new(unix_stream);
                TcpOrUnixSocket::Unix(io)
            }
        };

        let http_request_handler = HttpRequestHandler::new(connection_addr, configuration);

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_to_dispose = http_request_handler.clone();

        match io {
            TcpOrUnixSocket::Tcp(io) => {
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
            }
            TcpOrUnixSocket::Unix(io) => {
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
            }
        }

        if let Some(port) = port.as_ref() {
            crate::app::APP_CTX
                .metrics
                .update(|itm| itm.connection_by_port.dec(port))
                .await;
        }

        crate::app::APP_CTX
            .prometheus
            .dec_http2_server_connections(listening_addr_str.as_str());
        http_request_handler_to_dispose.dispose().await;
    });
}
