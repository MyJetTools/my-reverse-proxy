use std::{net::SocketAddr, sync::Arc};

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::app::AppContext;

use super::handle_request::HttpRequestHandler;

pub fn start_h2_server(addr: SocketAddr, app: Arc<AppContext>) {
    println!("Listening h2 on http://{}", addr);
    tokio::spawn(start_https2_server_loop(addr, app));
}

async fn start_https2_server_loop(listening_addr: SocketAddr, app: Arc<AppContext>) {
    let listener = tokio::net::TcpListener::bind(listening_addr).await.unwrap();
    let builder = Arc::new(hyper::server::conn::http2::Builder::new(
        TokioExecutor::new(),
    ));
    loop {
        let accepted_connection = listener.accept().await;

        if app.states.is_shutting_down() {
            println!("Shutting down h2 server");
            break;
        }

        if let Err(err) = &accepted_connection {
            println!(
                "Error accepting connection {}. Err: {:?}",
                listening_addr, err
            );
            continue;
        }

        let (stream, socket_addr) = accepted_connection.unwrap();

        let app = app.clone();
        let builder = builder.clone();

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
                        super::handle_request::handle_request(
                            http_request_handler.clone(),
                            req,
                            app.connection_settings.remote_connect_timeout,
                        )
                    }),
                )
                .await;

            http_request_handler_to_dispose.dispose().await;
        });
    }
}
