use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use rust_extensions::StopWatch;
use tokio::sync::Mutex;

use crate::{
    app::AppContext,
    http_proxy_pass::{HostPort, HttpProxyPass},
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

    pub fn new(proxy_pass: HttpProxyPass, app: Arc<AppContext>) -> Self {
        app.http_connections
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self::Direct { proxy_pass, app }
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
            HttpRequestHandler::Direct { proxy_pass, app: _ } => proxy_pass.dispose().await,
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
                let http_endpoint_info = app
                    .get_current_app_configuration()
                    .await
                    .get_http_endpoint_info(*listen_port, req.get_host().unwrap());

                match http_endpoint_info {
                    Ok(endpoint_info) => {
                        if endpoint_info.debug {
                            println!(
                                "Detected. {}: [{}]{:?}",
                                endpoint_info.as_str(),
                                req.method(),
                                req.uri()
                            );
                        }

                        let listening_port_info =
                            endpoint_info.get_listening_port_info(*socket_addr);

                        let http_proxy_pass = Arc::new(HttpProxyPass::new(
                            app,
                            endpoint_info,
                            listening_port_info,
                            None,
                        ));

                        proxy_pass_result = Some(http_proxy_pass);
                        *proxy_pass.lock().await = proxy_pass_result.clone();
                    }
                    Err(err) => {
                        let content = super::generate_layout(400, err.as_str(), None);

                        return Ok(hyper::Response::builder()
                            .status(hyper::StatusCode::BAD_REQUEST)
                            .body(
                                Full::new(content)
                                    .map_err(|e| crate::to_hyper_error(e))
                                    .boxed(),
                            )
                            .unwrap());
                    }
                }
            }

            let proxy_pass_result = proxy_pass_result.unwrap();

            handle_requests(app, req, &proxy_pass_result).await
        }
        HttpRequestHandler::Direct { proxy_pass, app } => {
            handle_requests(app, req, proxy_pass).await
        }
    }
}

async fn handle_requests(
    app: &Arc<AppContext>,
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: &HttpProxyPass,
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

    match proxy_pass.send_payload(&app, req).await {
        Ok(response) => {
            match response.as_ref() {
                Ok(response) => {
                    if let Some((req_str, mut sw)) = debug {
                        sw.pause();
                        println!(
                            "Res: {}->{} {}",
                            req_str,
                            response.status(),
                            sw.duration_as_string()
                        );
                    }
                }
                Err(err) => {
                    if let Some((req_str, mut sw)) = debug {
                        sw.pause();
                        println!("Resp: {}->{} {}", req_str, err, sw.duration_as_string());
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
