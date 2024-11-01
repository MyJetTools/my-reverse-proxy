use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Full};

pub fn into_full_body_response(
    response: hyper::Response<Full<Bytes>>,
) -> hyper::Response<BoxBody<Bytes, String>> {
    let (parts, body) = response.into_parts();

    let body = body.map_err(|e| crate::to_hyper_error(e)).boxed();
    hyper::Response::from_parts(parts, body)
}

pub fn into_empty_body(
    builder: http::response::Builder,
) -> hyper::Response<BoxBody<Bytes, String>> {
    let body = Full::new(Bytes::new());
    let body = body.map_err(|e| crate::to_hyper_error(e)).boxed();
    builder.body(body).unwrap()
}
