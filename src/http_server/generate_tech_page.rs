use bytes::Bytes;
use http_body_util::Full;

use crate::app::APP_VERSION;
use crate::http_proxy_pass::ProxyPassError;

pub fn generate_tech_page(err: ProxyPassError) -> hyper::Response<Full<Bytes>> {
    match err {
        ProxyPassError::Timeout => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::from(generate_layout(500, "Timeout")))
                .unwrap();
        }
        ProxyPassError::NoLocationFound => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(Full::from(generate_layout(404, "Not found")))
                .unwrap();
        }

        ProxyPassError::Unauthorized => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::UNAUTHORIZED)
                .body(Full::from(Bytes::from(generate_layout(
                    401,
                    "Unauthorized request",
                ))))
                .unwrap();
        }
        _ => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::from(Bytes::from(generate_layout(
                    500,
                    "Internal Server Error",
                ))))
                .unwrap();
        }
    }
}

fn generate_layout(status_code: u16, text: &str) -> Vec<u8> {
    format!(
        r#"
        <div style="text-align: center;">
        <h2>{text}</h2>
        <p>{status_code}</p>
        <hr/>
        <div>MyReverseProxy {APP_VERSION}</div>
        </div>
        "#
    )
    .into_bytes()
}
