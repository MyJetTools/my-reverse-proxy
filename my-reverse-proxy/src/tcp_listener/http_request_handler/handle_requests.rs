use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::BodyExt;
use http_body_util::Full;
use rust_extensions::StopWatch;

use crate::{
    http_proxy_pass::{HttpProxyPass, ProxyPassError},
    types::ConnectionIp,
};

pub async fn handle_requests(
    req: hyper::Request<hyper::body::Incoming>,
    proxy_pass: &HttpProxyPass,
    connection_ip: ConnectionIp,
) -> hyper::Result<hyper::Response<BoxBody<Bytes, String>>> {
    let request_host_for_metric: Option<String> = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.uri().host().map(|s| s.to_string()))
        .map(|s| {
            let host_no_port = match s.find(':') {
                Some(idx) => &s[..idx],
                None => s.as_str(),
            };
            host_no_port.trim().to_string()
        });

    if let Some(domain) = proxy_pass
        .endpoint_info
        .tracked_domain(request_host_for_metric.as_deref())
    {
        crate::app::APP_CTX.rps.inc_domain(domain);
    }

    let endpoint = proxy_pass.endpoint_info.host_endpoint.as_str();
    let endpoint_debug = crate::app::APP_CTX.debug_flags.is_endpoint_debug(endpoint);
    let ip = connection_ip.get_ip_log();
    let req_str: String = format!("[{}]{:?}", req.method(), req.uri());
    let sw = StopWatch::new();
    if endpoint_debug {
        crate::app::APP_CTX.proxy_logs.write(
            endpoint,
            None,
            ip.clone(),
            format!("Req: {}", req_str),
        );
    }

    match proxy_pass
        .send_payload(req, connection_ip, proxy_pass.endpoint_info.debug)
        .await
    {
        Ok(response) => {
            if endpoint_debug {
                match response.as_ref() {
                    Ok(response) => {
                        crate::app::APP_CTX.proxy_logs.write(
                            endpoint,
                            None,
                            ip.clone(),
                            format!(
                                "Response: {}->{} {}",
                                req_str,
                                response.status(),
                                sw.duration_as_string()
                            ),
                        );
                    }
                    Err(err) => {
                        crate::app::APP_CTX.proxy_logs.write(
                            endpoint,
                            None,
                            ip.clone(),
                            format!(
                                "Response Error: {}->{} {}",
                                req_str,
                                err,
                                sw.duration_as_string()
                            ),
                        );
                    }
                }
            }

            return response;
        }
        Err(err) => {
            if endpoint_debug {
                crate::app::APP_CTX.proxy_logs.write(
                    endpoint,
                    None,
                    ip.clone(),
                    format!(
                        "Tech Resp: {}->{:?} {}",
                        req_str,
                        err,
                        sw.duration_as_string()
                    ),
                );
            }
            if matches!(err, ProxyPassError::DropConnection) {
                let body: BoxBody<Bytes, String> = Full::new(Bytes::new())
                    .map_err(|never| match never {})
                    .boxed();
                let response = hyper::Response::builder()
                    .status(hyper::StatusCode::FORBIDDEN)
                    .header(hyper::header::CONNECTION, "close")
                    .body(body)
                    .unwrap();
                return Ok(response);
            }
            return Ok(super::utils::generate_tech_page(
                err,
                crate::app::APP_CTX.show_error_description.get_value(),
            ));
        }
    }
}
