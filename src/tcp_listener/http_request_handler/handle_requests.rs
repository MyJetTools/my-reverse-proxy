use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use rust_extensions::StopWatch;

use crate::{http_proxy_pass::HttpProxyPass, types::ConnectionIp};

pub async fn handle_requests(
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: &HttpProxyPass,
    connection_ip: ConnectionIp,
) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
    let debug = if proxy_pass.endpoint_info.debug {
        let req_str: String = format!(
            "{}: [{}]{:?}",
            proxy_pass.endpoint_info.as_str(),
            req.method(),
            req.uri()
        );
        let sw = StopWatch::new();

        println!("Req: {}", req_str);
        Some((req_str, sw))
    } else {
        None
    };

    match proxy_pass
        .send_payload(req, connection_ip, proxy_pass.endpoint_info.debug)
        .await
    {
        Ok(response) => {
            match response.as_ref() {
                Ok(response) => {
                    if let Some((req_str, sw)) = debug {
                        println!(
                            "Response: {}->{} {}",
                            req_str,
                            response.status(),
                            sw.duration_as_string()
                        );
                    }
                }
                Err(err) => {
                    if let Some((req_str, sw)) = debug {
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
            if let Some((req_str, sw)) = debug {
                println!(
                    "Tech Resp: {}->{:?} {}",
                    req_str,
                    err,
                    sw.duration_as_string()
                );
            }
            return Ok(super::utils::generate_tech_page(
                err,
                crate::app::APP_CTX.show_error_description.get_value(),
            ));
        }
    }
}
