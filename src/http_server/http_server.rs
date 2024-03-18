use std::{net::SocketAddr, sync::Arc};

use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{app::AppContext, http_server::ProxyPassError};

use super::ProxyPassClient;

pub struct HttpServer {
    pub addr: SocketAddr,
}

impl HttpServer {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
    pub fn start(&self, app: Arc<AppContext>) {
        println!("Listening on http://{}", self.addr);
        tokio::spawn(start_http_server(self.addr, app));
    }
}

async fn start_http_server(addr: SocketAddr, app: Arc<AppContext>) {
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    loop {
        let (stream, socket_addr) = listener.accept().await.unwrap();

        let io = TokioIo::new(stream);

        app.http_connections
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let http_proxy_pass = Arc::new(ProxyPassClient::new(socket_addr));

        let http_proxy_pass_to_dispose = http_proxy_pass.clone();

        let app = app.clone();

        let app_disposed = app.clone();

        let connection = http1
            .serve_connection(
                io,
                service_fn(move |req| handle_requests(req, http_proxy_pass.clone(), app.clone())),
            )
            .with_upgrades();

        tokio::task::spawn(async move {
            if let Err(err) = connection.await {
                println!(
                    "{}. Error serving connection: {:?}",
                    DateTimeAsMicroseconds::now().to_rfc3339(),
                    err
                );
            }

            http_proxy_pass_to_dispose.dispose().await;

            app_disposed
                .http_connections
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        });
    }
}

pub async fn handle_requests(
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: Arc<ProxyPassClient>,
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
                    println!("Error: {:?}", err);

                    return Ok(hyper::Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Full::from(Bytes::from("Internal Server Error")))
                        .unwrap());
                }
            }
        }
    }
}
