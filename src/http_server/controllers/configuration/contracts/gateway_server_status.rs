use std::sync::Arc;

use my_http_server::macros::MyHttpObjectStructure;
use serde::*;

use crate::{app::AppContext, tcp_gateway::TcpGatewayConnection};

#[derive(MyHttpObjectStructure, Serialize, Debug)]
pub struct GatewayServerStatus {
    pub connections: Vec<GatewayConnection>,
}

impl GatewayServerStatus {
    pub async fn new(app: &AppContext) -> Option<Self> {
        let server_gateway = app.gateway_server.as_ref()?;

        let result = Self {
            connections: GatewayConnection::new(
                server_gateway.get_gateway_connections().await.as_slice(),
            )
            .await,
        };

        Some(result)
    }
}

#[derive(MyHttpObjectStructure, Serialize, Debug)]
pub struct GatewayConnection {
    pub name: String,
    pub forward_connections: usize,
    pub proxy_connections: usize,
}

impl GatewayConnection {
    pub async fn new(connections: &[Arc<TcpGatewayConnection>]) -> Vec<GatewayConnection> {
        let mut result = Vec::new();

        for connection in connections {
            result.push(Self {
                name: connection.get_gateway_id().await.to_string(),
                forward_connections: connection.get_forward_connections_amount().await,
                proxy_connections: connection.get_forward_proxy_connections_amount().await,
            });
        }

        result
    }
}
