use my_http_server::macros::MyHttpObjectStructure;
use serde::*;

use crate::app::AppContext;

use super::GatewayConnection;

#[derive(MyHttpObjectStructure, Serialize, Debug)]
pub struct GatewayClientStatus {
    pub name: String,
    pub connections: Vec<GatewayConnection>,
}

impl GatewayClientStatus {
    pub async fn new(app: &AppContext) -> Vec<Self> {
        let mut result = Vec::new();
        for (name, client_gateway) in &app.gateway_clients {
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
