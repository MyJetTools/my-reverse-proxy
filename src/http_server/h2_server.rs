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
        let (stream, socket_addr) = listener.accept().await.unwrap();

        let io = TokioIo::new(stream);

        let app = app.clone();

        let builder = builder.clone();

        tokio::spawn(async move {
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
