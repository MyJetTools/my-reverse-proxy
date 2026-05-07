use bytes::Bytes;
use http::Response;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::body::Incoming;

pub fn into_full_body_response(
    response: hyper::Response<Full<Bytes>>,
) -> hyper::Response<BoxBody<Bytes, String>> {
    let (parts, body) = response.into_parts();

    let body = body.map_err(|e| e.to_string()).boxed();
    hyper::Response::from_parts(parts, body)
}

pub fn into_empty_body(
    builder: http::response::Builder,
) -> hyper::Response<BoxBody<Bytes, String>> {
    let body = Full::new(Bytes::new());
    let body = body.map_err(|e| e.to_string()).boxed();
    builder.body(body).unwrap()
}

pub fn from_incoming_body(response: Response<Incoming>) -> Response<BoxBody<Bytes, String>> {
    let (parts, body) = response.into_parts();

    let box_body = body.map_err(|e| e.to_string()).boxed();

    Response::from_parts(parts, box_body)
}

pub fn into_body(
    builder: http::response::Builder,
    body: Vec<u8>,
) -> http::Response<BoxBody<Bytes, String>> {
    let full_body = http_body_util::Full::new(hyper::body::Bytes::from(body));
    builder
        .body(full_body.map_err(|itm| itm.to_string()).boxed())
        .unwrap()
}
