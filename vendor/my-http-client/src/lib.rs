pub mod http1;

mod error;
pub use error::*;

pub mod http2;
mod my_http_client_connector;
pub mod utils;
pub use my_http_client_connector::*;
mod my_http_client_disconnect;
pub use my_http_client_disconnect::*;

pub mod http1_hyper;
pub mod hyper;

pub type HyperResponse = http::Response<http_body_util::combinators::BoxBody<bytes::Bytes, String>>;
mod headers;
pub use headers::*;

const CL_CR: &[u8] = b"\r\n";
pub extern crate http;

mod task_metrics;
pub use task_metrics::*;
