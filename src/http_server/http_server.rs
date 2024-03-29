use std::{net::SocketAddr, sync::Arc};

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use crate::app::AppContext;

use crate::http_proxy_pass::*;

pub fn start_http_server(
    addr: SocketAddr,
    app: Arc<AppContext>,
    endpoint_info: ProxyPassEndpointInfo,
) {
    println!("Listening http1 on http://{}", addr);
    tokio::spawn(start_http_server_loop(addr, app, endpoint_info));
}

async fn start_http_server_loop(
    addr: SocketAddr,
    app: Arc<AppContext>,
    endpoint_info: ProxyPassEndpointInfo,
) {
    let endpoint_info = Arc::new(endpoint_info);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let mut http1 = http1::Builder::new();
    http1.keep_alive(true);

    loop {
        let (stream, socket_addr) = listener.accept().await.unwrap();

        let io = TokioIo::new(stream);

        app.http_connections
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let modify_headers_settings = app
            .settings_reader
            .get_http_endpoint_modify_headers_settings(endpoint_info.as_ref())
            .await;

        let endpoint_info = endpoint_info.clone();

        let http_proxy_pass = Arc::new(HttpProxyPass::new(
            socket_addr,
            modify_headers_settings,
            endpoint_info,
        ));

        let http_proxy_pass_to_dispose = http_proxy_pass.clone();

        let app = app.clone();

        let app_disposed = app.clone();

        let connection = http1
            .serve_connection(
                io,
                service_fn(move |req| {
                    super::handle_request::handle_requests(
                        req,
                        http_proxy_pass.clone(),
                        app.clone(),
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

            http_proxy_pass_to_dispose.dispose().await;

            app_disposed
                .http_connections
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
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
