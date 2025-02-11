use std::sync::Arc;

use rust_extensions::remote_endpoint::RemoteEndpointOwned;

pub struct HttpOverGatewayConnection {
    pub remote_endpoint: RemoteEndpointOwned,
    pub domain_name: Option<String>,
    pub debug: bool,
    pub gateway_id: Arc<String>,
}

impl HttpOverGatewayConnection {
    pub fn new() -> Self {
        todo!("Implement")
    }
}
