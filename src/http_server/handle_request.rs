use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use rust_extensions::StopWatch;

use crate::{app::AppContext, http_proxy_pass::HttpProxyPass};

pub async fn handle_requests(
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: Arc<HttpProxyPass>,
    app: Arc<AppContext>,
) -> hyper::Result<hyper::Response<Full<Bytes>>> {
    let debug = if proxy_pass.endpoint_info.debug {
        let req_str = format!(
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
                        println!("Res: {}->{} {}", req_str, err, sw.duration_as_string());
                    }
                }
            }

            return response;
        }
        Err(err) => {
            return Ok(super::generate_tech_page(err));
        }
    }
}
