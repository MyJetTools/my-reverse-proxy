use my_http_server::macros::MyHttpObjectStructure;
use serde::*;

use super::GatewayConnection;

#[derive(MyHttpObjectStructure, Serialize, Debug)]
pub struct GatewayClientStatus {
    pub name: String,
    pub connections: Vec<GatewayConnection>,
}

impl GatewayClientStatus {
    pub async fn new() -> Vec<Self> {
        let mut result = Vec::new();
        for (name, client_gateway) in &crate::app::APP_CTX.gateway_clients {
            let itm = Self {
                name: name.to_string(),
                connections: GatewayConnection::new(
                    client_gateway.get_gateway_connections().await.as_slice(),
                )
                .await,
            };

            result.push(itm);
        }

        result
    }
}
