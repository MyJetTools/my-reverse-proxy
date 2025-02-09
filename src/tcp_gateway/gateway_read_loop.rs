use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::{io::AsyncReadExt, net::tcp::OwnedReadHalf};

use super::*;

pub async fn read_loop<TTcpGatewayConnection: TcpGatewayConnection + Send + Sync + 'static>(
    tcp_gateway: Arc<TcpGatewayInner>,
    mut read: OwnedReadHalf,
    gateway_connection: Arc<TTcpGatewayConnection>,
    packet_handler: impl TcpGatewayPacketHandler<GateWayConnection = TTcpGatewayConnection>,
) {
    let mut buf = super::create_read_loop();

    loop {
        let mut payload_size = [0u8; 4];
        let read_result = read.read_exact(&mut payload_size).await;
        let now = DateTimeAsMicroseconds::now();
        gateway_connection.set_last_incoming_payload_time(now);

        match read_result {
            Ok(result) => {
                if result != payload_size.len() {
                    println!("[1] TCP Gateway is disconnected");
                    break;
                }
            }
            Err(err) => {
                println!(
                    "[1] Failed to read payload size from TCP Gateway at {}. Err: {:?}",
                    tcp_gateway.addr.as_str(),
                    err
                );
                break;
            }
        }

        let payload_size = u32::from_le_bytes(payload_size) as usize;

        let payload = &mut buf[0..payload_size];

        let read_result = read.read_exact(payload).await;

        match read_result {
            Ok(result) => {
                if result != payload.len() {
                    println!("[2] TCP Gateway is disconnected");
                    break;
                }
            }
            Err(err) => {
                println!(
                    "[2] Failed to read payload size from TCP Gateway at {}. Err: {:?}",
                    tcp_gateway.addr.as_str(),
                    err
                );
                break;
            }
        };

        match TcpGatewayContract::parse(payload) {
            Ok(packet) => {
                let now = DateTimeAsMicroseconds::now();
                gateway_connection.set_last_incoming_payload_time(now);
                packet_handler
                    .handle_packet(packet, &gateway_connection)
                    .await
            }
            Err(err) => {
                println!(
                    "Failed to handle packet from TCP Gateway at {}. Err: {:?}",
                    tcp_gateway.addr.as_str(),
                    err
                );
                break;
            }
        }
    }

    gateway_connection.disconnect().await;
}

#[async_trait::async_trait]
pub trait TcpGatewayConnection {
    type ForwardConnection: TcpGatewayForwardConnection;

    fn get_addr(&self) -> &str;

    fn set_last_incoming_payload_time(&self, time: DateTimeAsMicroseconds);
    fn get_last_incoming_payload_time(&self) -> DateTimeAsMicroseconds;

    async fn disconnect(&self);
    async fn send_payload(&self, payload: &[u8]) -> bool;

    async fn add_forward_connection(
        &self,
        connection_id: u32,
        connection: Arc<Self::ForwardConnection>,
    );

    async fn get_forward_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<Self::ForwardConnection>>;

    async fn has_forward_connection(&self, connection_id: u32) -> bool;

    async fn remove_forward_connection(
        &self,
        connection_id: u32,
    ) -> Option<Arc<Self::ForwardConnection>>;
}

#[async_trait::async_trait]
pub trait TcpGatewayPacketHandler {
    type GateWayConnection: TcpGatewayConnection;
    async fn handle_packet<'d>(
        &self,
        contract: TcpGatewayContract<'d>,
        connection: &Arc<Self::GateWayConnection>,
    );
}
