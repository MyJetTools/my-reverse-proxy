use bytes::Bytes;
use http_body_util::Full;

use crate::app::APP_VERSION;
use crate::http_proxy_pass::ProxyPassError;

pub fn generate_tech_page(err: ProxyPassError) -> hyper::Response<Full<Bytes>> {
    match err {
        ProxyPassError::Timeout => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::from(generate_layout(500, "Timeout", None)))
                .unwrap();
        }
        ProxyPassError::NoLocationFound => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(Full::from(generate_layout(404, "Not found", None)))
                .unwrap();
        }

        ProxyPassError::Unauthorized => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::UNAUTHORIZED)
                .body(Full::from(Bytes::from(generate_layout(
                    401,
                    "Unauthorized request",
                    None,
                ))))
                .unwrap();
        }

        ProxyPassError::UserIsForbidden => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::FORBIDDEN)
                .body(Full::from(Bytes::from(generate_layout(
                    403,
                    "Access is forbidden",
                    None,
                ))))
                .unwrap();
        }

        ProxyPassError::IpRestricted(ip) => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::UNAUTHORIZED)
                .body(Full::from(Bytes::from(generate_layout(
                    401,
                    "Restricted by IP",
                    Some(ip.as_str()),
                ))))
                .unwrap();
        }
        _ => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::from(Bytes::from(generate_layout(
                    500,
                    "Internal Server Error",
                    None,
                ))))
                .unwrap();
        }
    }
}

pub fn generate_layout(status_code: u16, text: &str, second_line: Option<&str>) -> Vec<u8> {
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
}
