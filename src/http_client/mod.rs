mod http1_client;
pub use http1_client::*;
mod connect_to_http_endpoint;
pub use connect_to_http_endpoint::*;
mod error;
pub use error::*;
mod connect_to_tls_endpoint;
pub use connect_to_tls_endpoint::*;

mod connect_to_http_over_ssh;
pub use connect_to_http_over_ssh::*;
mod connect_to_http2_endpoint;
pub use connect_to_http2_endpoint::*;
mod http2_client;
pub use http2_client::*;
mod http_client;
pub use http_client::*;
mod connect_to_http2_over_ssh;
pub use connect_to_http2_over_ssh::*;

pub const HTTP_CLIENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
