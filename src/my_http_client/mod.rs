mod my_http_client;
pub use my_http_client::*;
//mod http_parser;
mod tcp_buffer;
pub use tcp_buffer::*;
mod body_reader;
pub use body_reader::*;
mod headers_reader;
pub use headers_reader::*;
mod detected_body_size;
pub use detected_body_size::*;
mod my_http_client_inner;
pub use my_http_client_inner::*;
mod read_loop;
mod write_loop;

mod queue_of_requests;
pub use queue_of_requests::*;

mod my_http_client_connector;
pub use my_http_client_connector::*;

mod error;
pub use error::*;

mod my_http_client_connection_context;
pub use my_http_client_connection_context::*;

mod my_http_request;
pub use my_http_request::*;

#[derive(Debug)]
pub enum HttpParseError {
    GetMoreData,
    Error(String),
}
