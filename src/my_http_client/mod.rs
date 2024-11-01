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
mod my_http_client_write_part;
pub use my_http_client_write_part::*;

#[derive(Debug)]
pub enum HttpParseError {
    GetMoreData,
    Error(String),
}
