use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};

use crate::app::APP_VERSION;
use crate::http_proxy_pass::ProxyPassError;

pub fn generate_tech_page(err: ProxyPassError) -> hyper::Response<BoxBody<Bytes, String>> {
    match err {
        ProxyPassError::Timeout => {
            let body: Bytes = generate_layout(504, "Timeout", None).into();
            let body = Full::new(body)
                .map_err(|e| crate::to_hyper_error(e))
                .boxed();
            return hyper::Response::builder()
                .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                .body(body)
                .unwrap();
        }
        ProxyPassError::NoLocationFound => {
            let body: Bytes = generate_layout(404, "Not found", None).into();

            return hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(
                    Full::from(body)
                        .map_err(|e| crate::to_hyper_error(e))
                        .boxed(),
                )
                .unwrap();
        }

        ProxyPassError::Unauthorized => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::UNAUTHORIZED)
                .body(
                    Full::from(generate_layout(401, "Unauthorized request", None))
                        .map_err(|e| crate::to_hyper_error(e))
                        .boxed(),
                )
                .unwrap();
        }

        ProxyPassError::UserIsForbidden => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::FORBIDDEN)
                .body(
                    Full::from(generate_layout(403, "Access is forbidden", None))
                        .map_err(|e| crate::to_hyper_error(e))
                        .boxed(),
                )
                .unwrap();
        }

        ProxyPassError::IpRestricted(ip) => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::UNAUTHORIZED)
                .body(
                    Full::from(generate_layout(401, "Restricted by IP", Some(ip.as_str())))
                        .map_err(|e| crate::to_hyper_error(e))
                        .boxed(),
                )
                .unwrap();
        }
        _ => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .body(
                    Full::from(generate_layout(500, "Internal Server Error", None))
                        .map_err(|e| crate::to_hyper_error(e))
                        .boxed(),
                )
                .unwrap();
        }
    }
}

pub fn generate_layout(status_code: u16, text: &str, second_line: Option<&str>) -> Bytes {
    let second_line = if let Some(second_line) = second_line {
        format!("<h4>{}</h4>", second_line)
    } else {
        "".to_string()
    };
    format!(
        r#"
        <div style="text-align: center;">
        <h2>{text}</h2>
      {second_line}
        <p>{status_code}</p>
        <hr/>
        <div>MyReverseProxy {APP_VERSION}</div>
        </div>
        "#
    )
    .into_bytes()
    .into()
}
