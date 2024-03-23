use std::{net::SocketAddr, sync::Arc};

use http_body_util::Full;
use hyper::{body::Bytes, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::app::AppContext;

use crate::http_proxy_pass::*;

pub fn start_http2_server(addr: SocketAddr, app: Arc<AppContext>, host_str: String) {
    println!("Listening http2 on https://{}", addr);
    tokio::spawn(start_http_server_loop(addr, app, host_str));
}

async fn start_http_server_loop(addr: SocketAddr, app: Arc<AppContext>, host_str: String) {
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let builder = Arc::new(hyper::server::conn::http2::Builder::new(
        TokioExecutor::new(),
    ));
    loop {
        let (stream, socket_addr) = listener.accept().await.unwrap();

        let io = TokioIo::new(stream);

        let app = app.clone();

        let builder = builder.clone();
        let modify_headers_settings = app
            .settings_reader
            .get_http_endpoint_modify_headers_settings(host_str.as_str())
            .await;

        tokio::spawn(async move {
            let http_proxy_pass = Arc::new(HttpProxyPass::new(
                socket_addr,
                modify_headers_settings,
                false,
            ));
            let proxy_pass_to_dispose = http_proxy_pass.clone();
            let _ = builder
                .serve_connection(
                    io,
                    service_fn(move |req| {
                        handle_requests(req, http_proxy_pass.clone(), app.clone())
                    }),
                )
                .await;

            proxy_pass_to_dispose.dispose().await;
        });
    }
}

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
