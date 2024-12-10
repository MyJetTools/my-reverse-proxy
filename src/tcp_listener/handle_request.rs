use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http::StatusCode;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use rust_extensions::StopWatch;
use tokio::sync::Mutex;

use crate::{
    app::AppContext,
    http_proxy_pass::{HostPort, HttpListenPortInfo, HttpProxyPass},
};

pub enum HttpRequestHandler {
    LazyInit {
        proxy_pass: Mutex<Option<Arc<HttpProxyPass>>>,
        app: Arc<AppContext>,
        listen_port: u16,
        socket_addr: SocketAddr,
    },
    Direct {
        proxy_pass: HttpProxyPass,
        app: Arc<AppContext>,
        socket_addr: SocketAddr,
    },
}

impl HttpRequestHandler {
    pub fn new_lazy(app: Arc<AppContext>, listen_port: u16, socket_addr: SocketAddr) -> Self {
        app.http_connections
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self::LazyInit {
            proxy_pass: Mutex::new(None),
            app,
            listen_port,
            socket_addr,
        }
    }

    pub fn new(proxy_pass: HttpProxyPass, app: Arc<AppContext>, socket_addr: SocketAddr) -> Self {
        app.http_connections
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self::Direct {
            proxy_pass,
            app,
            socket_addr,
        }
    }

    pub async fn dispose(&self) {
        match self {
            HttpRequestHandler::LazyInit {
                proxy_pass,
                app: _,
                listen_port: _,
                socket_addr: _,
            } => {
                let proxy_pass = proxy_pass.lock().await.clone();

                if let Some(proxy_pass) = proxy_pass {
                    proxy_pass.dispose().await;
                }
            }
            HttpRequestHandler::Direct {
                proxy_pass,
                app: _,
                socket_addr: _,
            } => proxy_pass.dispose().await,
        }
    }
}

pub async fn handle_request(
    handler: Arc<HttpRequestHandler>,
    req: hyper::Request<hyper::body::Incoming>,
) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
    match handler.as_ref() {
        HttpRequestHandler::LazyInit {
            proxy_pass,
            app,
            listen_port,
            socket_addr,
        } => {
            let mut proxy_pass_result = {
                let proxy_pass = proxy_pass.lock().await;
                proxy_pass.clone()
            };

            if proxy_pass_result.is_none() {
                let host = req.get_host();

                if host.is_none() {
                    println!(
                        "Can not detect host. Uri:{}. Headers: {:?}",
                        req.uri(),
                        req.headers()
                    );
                    return create_err_response(
                        StatusCode::BAD_REQUEST,
                        "Unknown host".to_string().into_bytes(),
                    );
                }

                let http_endpoint_info = app
                    .current_configuration
                    .get(|itm| itm.get_http_endpoint_info(*listen_port, host.unwrap()))
                    .await;

                if http_endpoint_info.is_none() {
                    let content = super::generate_layout(400, "No configuration found", None);
                    return create_err_response(StatusCode::BAD_REQUEST, content);
                }

                let http_endpoint_info = http_endpoint_info.unwrap();

                if http_endpoint_info.debug {
                    println!(
                        "Detected. {}: [{}]{:?}",
                        http_endpoint_info.as_str(),
                        req.method(),
                        req.uri()
                    );
                }

                let listening_port_info = HttpListenPortInfo {
                    endpoint_type: http_endpoint_info.listen_endpoint_type,
                    socket_addr: *socket_addr,
                };

                let http_proxy_pass =
                    HttpProxyPass::new(app, http_endpoint_info, listening_port_info, None).await;

                proxy_pass_result = Some(http_proxy_pass.into());
                *proxy_pass.lock().await = proxy_pass_result.clone();
                //.get_http_endpoint_info(*listen_port, host.unwrap());
            }

            let proxy_pass_result = proxy_pass_result.unwrap();

            handle_requests(app, req, &proxy_pass_result, socket_addr).await
        }
        HttpRequestHandler::Direct {
            proxy_pass,
            app,
            socket_addr,
        } => handle_requests(app, req, proxy_pass, socket_addr).await,
    }
}

async fn handle_requests(
    app: &Arc<AppContext>,
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: &HttpProxyPass,
    socket_addr: &SocketAddr,
) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
    let mut sw = StopWatch::new();

    sw.start();

    let debug = if proxy_pass.endpoint_info.debug {
        let req_str: String = format!(
            "{}: [{}]{:?}",
            proxy_pass.endpoint_info.as_str(),
            req.method(),
            req.uri()
        );
        let mut sw = StopWatch::new();
        sw.start();
        println!("Req: {}", req_str);
        Some((req_str, sw))
    } else {
        None
    };

    match proxy_pass.send_payload(&app, req, socket_addr).await {
        Ok(response) => {
            match response.as_ref() {
                Ok(response) => {
                    if let Some((req_str, mut sw)) = debug {
                        sw.pause();
                        println!(
                            "Response: {}->{} {}",
                            req_str,
                            response.status(),
                            sw.duration_as_string()
                        );
                    }
                }
                Err(err) => {
                    if let Some((req_str, mut sw)) = debug {
                        sw.pause();
                        println!(
                            "Response Error: {}->{} {}",
                            req_str,
                            err,
                            sw.duration_as_string()
                        );
                    }
                }
            }

            return response;
        }
        Err(err) => {
            if let Some((req_str, mut sw)) = debug {
                sw.pause();
                println!(
                    "Tech Resp: {}->{:?} {}",
                    req_str,
                    err,
                    sw.duration_as_string()
                );
            }
            return Ok(super::generate_tech_page(
                err,
                app.show_error_description.get_value(),
            ));
        }
    }
}

fn create_err_response(
    status_code: StatusCode,
    content: impl Into<Bytes>,
) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
    let result = hyper::Response::builder()
        .status(status_code)
        .body(
            Full::new(content.into())
                .map_err(|e| crate::to_hyper_error(e))
                .boxed(),
        )
        .unwrap();

    Ok(result)
}
