use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use crate::app::AppContext;

use super::handle_request::HttpRequestHandler;

pub fn start_http_server(addr: SocketAddr, app: Arc<AppContext>, debug: bool) {
    println!("Listening http1 on http://{}", addr);
    tokio::spawn(start_http_server_loop(addr, app, debug));
}

async fn start_http_server_loop(listening_addr: SocketAddr, app: Arc<AppContext>, debug: bool) {
    let listener = tokio::net::TcpListener::bind(listening_addr).await.unwrap();
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);
    let listening_addr_str = format!("http://{}", listening_addr);
    let listening_addr_str = Arc::new(listening_addr_str);
    loop {
        let accepted_connection = listener.accept().await;

        if debug {
            println!("New connection accepted");
        }

        if app.states.is_shutting_down() {
            println!("Shutting down http server");
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

        let io = TokioIo::new(stream);

        let http_request_handler =
            HttpRequestHandler::new_lazy(app.clone(), listening_addr.port(), socket_addr);

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

        app.prometheus
            .inc_http1_server_connections(listening_addr_str.as_str());

        let prometheus = app.prometheus.clone();
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

            prometheus.dec_http1_server_connections(listening_addr_str.as_str());
            http_request_handler_disposed.dispose().await;
        });
    }
}
