use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use rust_extensions::StopWatch;

use crate::{app::AppContext, http_proxy_pass::HttpProxyPass};

pub async fn handle_requests(
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
            return Ok(super::utils::generate_tech_page(
                err,
                app.show_error_description.get_value(),
            ));
        }
    }
}
