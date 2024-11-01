mod http1_client;
pub use http1_client::*;
mod error;
pub use error::*;

mod connect_to_http2_endpoint;
pub use connect_to_http2_endpoint::*;
mod http2_client;
pub use http2_client::*;

mod connect_to_http2_over_ssh;
pub use connect_to_http2_over_ssh::*;

pub const HTTP_CLIENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
mod connect_to_http2_unix_socket_endpoint;
mod http1_over_ssh_client;
pub use http1_over_ssh_client::*;
