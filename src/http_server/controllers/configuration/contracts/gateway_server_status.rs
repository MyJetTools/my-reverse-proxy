use std::sync::Arc;

use my_http_server::macros::MyHttpObjectStructure;
use serde::*;

use crate::tcp_gateway::TcpGatewayConnection;

#[derive(MyHttpObjectStructure, Serialize, Debug)]
pub struct GatewayServerStatus {
    pub connections: Vec<GatewayConnection>,
}

impl GatewayServerStatus {
    pub async fn new() -> Option<Self> {
        let server_gateway = crate::app::APP_CTX.gateway_server.as_ref()?;

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
    pub ping_time: String,
    pub is_incoming_forward_connection_allowed: bool,
    pub in_history: Vec<usize>,
    pub out_history: Vec<usize>,
}

impl GatewayConnection {
    pub async fn new(connections: &[Arc<TcpGatewayConnection>]) -> Vec<GatewayConnection> {
        let mut result = Vec::new();

        for connection in connections {
            let ping = connection.last_ping_duration.to_duration();

            let (in_history, out_history) = {
                let metrics_access = connection.metrics.lock().await;
                let in_history = metrics_access.in_per_second.get_metrics();
                let out_history = metrics_access.out_per_second.get_metrics();
                (in_history, out_history)
            };

            result.push(Self {
                name: connection.get_gateway_id().await.to_string(),
                forward_connections: connection.get_forward_connections_amount().await,
                proxy_connections: connection.get_forward_proxy_connections_amount().await,
                ping_time: format!("{:?}", ping),
                is_incoming_forward_connection_allowed: connection
                    .is_incoming_forward_connection_allowed(),
                in_history,
                out_history,
            });
        }

        result
    }
}
