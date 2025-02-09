use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::{io::AsyncReadExt, net::tcp::OwnedReadHalf};

use super::*;

pub async fn read_loop(
    tcp_gateway: Arc<TcpGatewayInner>,
    mut read: OwnedReadHalf,
    gateway_connection: Arc<TcpGatewayConnection>,
    packet_handler: impl TcpGatewayPacketHandler,
    debug: bool,
) {
    let mut buf = crate::tcp_utils::allocated_read_buffer();

    loop {
        let mut payload_size = [0u8; 4];
        let read_result = read.read_exact(&mut payload_size).await;
        let now = DateTimeAsMicroseconds::now();
        gateway_connection.set_last_incoming_payload_time(now);

        match read_result {
            Ok(result) => {
                if result != payload_size.len() {
                    if debug {
                        println!("[1] TCP Gateway is disconnected");
                    }

                    break;
                }
            }
            Err(err) => {
                if debug {
                    println!(
                        "[1] Failed to read payload size from TCP Gateway at {}. Err: {:?}",
                        tcp_gateway.addr.as_str(),
                        err
                    );
                }

                break;
            }
        }

        let payload_size = u32::from_le_bytes(payload_size) as usize;

        let payload = &mut buf[0..payload_size];

        let read_result = read.read_exact(payload).await;

        match read_result {
            Ok(result) => {
                if result != payload.len() {
                    if debug {
                        println!("[2] TCP Gateway is disconnected");
                    }
                    break;
                }
            }
            Err(err) => {
                if debug {
                    println!(
                        "[2] Failed to read payload size from TCP Gateway at {}. Err: {:?}",
                        tcp_gateway.addr.as_str(),
                        err
                    );
                }

                break;
            }
        };

        match TcpGatewayContract::parse(payload) {
            Ok(packet) => {
                let now = DateTimeAsMicroseconds::now();
                gateway_connection.set_last_incoming_payload_time(now);
                packet_handler
                    .handle_packet(packet, &tcp_gateway, &gateway_connection)
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

    let gateway_id = gateway_connection.get_gateway_id().await;
    tcp_gateway
        .set_gateway_connection(gateway_id.as_str(), None)
        .await;
    gateway_connection.disconnect_gateway().await;
}
