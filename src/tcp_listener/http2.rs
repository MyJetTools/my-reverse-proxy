use std::{net::SocketAddr, sync::Arc};

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::{
    configurations::HttpListenPortConfiguration, network_stream::*,
    tcp_listener::AcceptedTcpConnection,
};

use super::http_request_handler::http::HttpRequestHandler;

pub async fn handle_connection(
    accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<HttpListenPortConfiguration>,
) {
    let listening_addr_str = format!("http://{}", listening_addr);

    let http2_builder = Arc::new(hyper::server::conn::http2::Builder::new(
        TokioExecutor::new(),
    ));

    let builder = http2_builder.clone();

    crate::app::APP_CTX
        .prometheus
        .inc_http1_server_connections(listening_addr_str.as_str());

    crate::app::APP_CTX
        .metrics
        .update(|itm| itm.connection_by_port.inc(&listening_addr.port()))
        .await;

    let listening_addr_str = listening_addr_str.clone();

    crate::app::APP_CTX
        .prometheus
        .inc_http2_server_connections(listening_addr_str.as_str());
    tokio::spawn(async move {
        let io = match accepted_connection.network_stream {
            MyNetworkStream::Tcp(tcp_stream) => {
                let io = TokioIo::new(tcp_stream);

                TcpOrUnixSocket::Tcp(io)
            }
            MyNetworkStream::UnixSocket(unix_stream) => {
                let io = TokioIo::new(unix_stream);
                TcpOrUnixSocket::Unix(io)
            }
        };

        let http_request_handler = HttpRequestHandler::new(accepted_connection.addr, configuration);

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

        crate::app::APP_CTX
            .metrics
            .update(|itm| itm.connection_by_port.dec(&listening_addr.port()))
            .await;

        crate::app::APP_CTX
            .prometheus
            .dec_http2_server_connections(listening_addr_str.as_str());
        http_request_handler_to_dispose.dispose().await;
    });
}
