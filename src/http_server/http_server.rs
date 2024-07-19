use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use crate::app::AppContext;

use super::handle_request::HttpRequestHandler;

pub fn start_http_server(addr: SocketAddr, app: Arc<AppContext>) {
    println!("Listening http1 on http://{}", addr);
    tokio::spawn(start_http_server_loop(addr, app));
}

async fn start_http_server_loop(listening_addr: SocketAddr, app: Arc<AppContext>) {
    let listener = tokio::net::TcpListener::bind(listening_addr).await.unwrap();
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    let request_timeout = app.connection_settings.remote_connect_timeout;

    loop {
        let accepted_connection = listener.accept().await;

        println!("New connection accepted");
        if app.states.is_shutting_down() {
            println!("Shutting down http server");
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

        let io = TokioIo::new(stream);

        let http_request_handler =
            HttpRequestHandler::new_lazy(app.clone(), listening_addr.port(), socket_addr);

        let http_request_handler = Arc::new(http_request_handler);

        let http_request_handler_disposed = http_request_handler.clone();

        let connection = http1
            .serve_connection(
                io,
                service_fn(move |req| {
                    super::handle_request::handle_request(
                        http_request_handler.clone(),
                        req,
                        request_timeout,
                    )
                }),
            )
            .with_upgrades();

        tokio::task::spawn(async move {
            if let Err(_) = connection.await {
                /*
                println!(
                    "{}. Error serving connection: {:?}",
                    DateTimeAsMicroseconds::now().to_rfc3339(),
                    err
                );
                 */
            }

            http_request_handler_disposed.dispose().await;
        });
    }
}

/*
pub async fn handle_requests(
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: Arc<HttpProxyPass>,
    app: Arc<AppContext>,
) -> hyper::Result<hyper::Response<Full<Bytes>>> {




    match proxy_pass.send_payload(&app, req).await {
        Ok(response) => return response,
        Err(err) => {
            if err.is_timeout() {
                return Ok(hyper::Response::builder()
                    .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Full::from(Bytes::from("Timeout")))
                    .unwrap());
            }

            match err {
                ProxyPassError::NoLocationFound => {
                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::NOT_FOUND)
                        .body(Full::from(Bytes::from("Not Found")))
                        .unwrap());
                }
                _ => {
                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Full::from(Bytes::from("Internal Server Error")))
                        .unwrap());
                }
            }
        }
    }
}
*/
