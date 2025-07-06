use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use crate::{configurations::HttpListenPortConfiguration, tcp_or_unix::*};

use super::{http_request_handler::http::HttpRequestHandler, AcceptedTcpConnection};

pub async fn handle_connection(
    accepted_connection: AcceptedTcpConnection,
    listening_addr: SocketAddr,
    configuration: Arc<HttpListenPortConfiguration>,
) {
    let listening_addr_str = format!("http://{}", listening_addr);

    let io = match accepted_connection.network_stream {
        MyNetworkStream::Tcp(tcp_stream) => {
            let io = TokioIo::new(tcp_stream);
            TcpOrUnixSocket::Tcp(io)
        }
        #[cfg(unix)]
        MyNetworkStream::UnixSocket(unix_stream) => {
            let io = TokioIo::new(unix_stream);
            TcpOrUnixSocket::Unix(io)
        }

        #[cfg(unix)]
        MyNetworkStream::Ssh(_) => {
            panic!("Http server does not work with ssh network stream");
        }
    };

    let http_request_handler = HttpRequestHandler::new(accepted_connection.addr, configuration);

    let http_request_handler = Arc::new(http_request_handler);

    let http_request_handler_disposed = http_request_handler.clone();

    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    let connection = match io {
        TcpOrUnixSocket::Tcp(io) => {
            let tcp_connection = http1
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        super::http_request_handler::http::handle_request(
                            http_request_handler.clone(),
                            req,
                        )
                    }),
                )
                .with_upgrades()
                .into();
            TcpOrUnixSocket::Tcp(tcp_connection)
        }
        #[cfg(unix)]
        TcpOrUnixSocket::Unix(io) => {
            let unix_connection = http1
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        super::http_request_handler::http::handle_request(
                            http_request_handler.clone(),
                            req,
                        )
                    }),
                )
                .with_upgrades()
                .into();

            TcpOrUnixSocket::Unix(unix_connection)
        }
    };

    crate::app::APP_CTX
        .prometheus
        .inc_http1_server_connections(listening_addr_str.as_str());

    crate::app::APP_CTX
        .metrics
        .update(|itm| itm.connection_by_port.inc(&listening_addr.port()))
        .await;

    let listening_addr_str = listening_addr_str.clone();

    tokio::task::spawn(async move {
        match connection {
            TcpOrUnixSocket::Tcp(tcp) => {
                if let Some(tcp) = tcp {
                    if let Err(err) = tcp.await {
                        println!(
                            "{}. Error serving connection: {:?}",
                            rust_extensions::date_time::DateTimeAsMicroseconds::now().to_rfc3339(),
                            err
                        );
                    }
                }
            }
            #[cfg(unix)]
            TcpOrUnixSocket::Unix(unix) => {
                if let Some(unix) = unix {
                    if let Err(err) = unix.await {
                        println!(
                            "{}. Error serving connection: {:?}",
                            rust_extensions::date_time::DateTimeAsMicroseconds::now().to_rfc3339(),
                            err
                        );
                    }
                }
            }
        }

        crate::app::APP_CTX
            .prometheus
            .dec_http1_server_connections(listening_addr_str.as_str());

        crate::app::APP_CTX
            .metrics
            .update(|itm| itm.connection_by_port.dec(&listening_addr.port()))
            .await;
        http_request_handler_disposed.dispose().await;
    });
}
