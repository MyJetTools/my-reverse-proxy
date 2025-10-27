use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};

use crate::http_proxy_pass::ProxyPassError;

pub fn generate_tech_page(
    err: ProxyPassError,
    show_error_description: bool,
) -> hyper::Response<BoxBody<Bytes, String>> {
    let second_line_error = if show_error_description {
        Some(format!("{err:?}").into())
    } else {
        None
    };

    match err {
        ProxyPassError::Timeout => {
            let body: Bytes =
                crate::error_templates::generate_layout(504, "Timeout", second_line_error).into();
            let body = Full::new(body)
                .map_err(|e| crate::to_hyper_error(e))
                .boxed();
            return hyper::Response::builder()
                .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                .body(body)
                .unwrap();
        }
        ProxyPassError::NoLocationFound => {
            let body: Bytes =
                crate::error_templates::generate_layout(404, "Not found", second_line_error).into();

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
                    Full::from(crate::error_templates::generate_layout(
                        401,
                        "Unauthorized request",
                        second_line_error,
                    ))
                    .map_err(|e| crate::to_hyper_error(e))
                    .boxed(),
                )
                .unwrap();
        }

        ProxyPassError::UserIsForbidden => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::FORBIDDEN)
                .body(
                    Full::from(crate::error_templates::generate_layout(
                        403,
                        "Access is forbidden",
                        second_line_error,
                    ))
                    .map_err(|e| crate::to_hyper_error(e))
                    .boxed(),
                )
                .unwrap();
        }

        ProxyPassError::IpRestricted(ip) => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::UNAUTHORIZED)
                .body(
                    Full::from(crate::error_templates::generate_layout(
                        401,
                        "Restricted by IP",
                        Some(ip.as_str().into()),
                    ))
                    .map_err(|e| crate::to_hyper_error(e))
                    .boxed(),
                )
                .unwrap();
        }
        _ => {
            return hyper::Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .body(
                    Full::from(crate::error_templates::generate_layout(
                        500,
                        "Internal Server Error",
                        second_line_error,
                    ))
                    .map_err(|e| crate::to_hyper_error(e))
                    .boxed(),
                )
                .unwrap();
        }
    }
}
