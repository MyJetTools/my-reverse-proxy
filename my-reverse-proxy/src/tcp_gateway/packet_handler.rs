use std::sync::Arc;

use super::{TcpGatewayConnection, TcpGatewayContract, TcpGatewayInner};

#[async_trait::async_trait]
pub trait TcpGatewayPacketHandler {
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        tcp_gateway: &Arc<TcpGatewayInner>,
        connection: &Arc<TcpGatewayConnection>,
    ) -> Result<(), String>;
}
