use std::sync::Arc;

use encryption::*;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::{io::AsyncReadExt, net::tcp::OwnedReadHalf};

use super::*;

const MAX_PAYLOAD_SIZE: usize = 1024 * 1024 * 5;

pub async fn read_loop(
    tcp_gateway: Arc<TcpGatewayInner>,
    mut read: OwnedReadHalf,
    gateway_connection: Arc<TcpGatewayConnection>,
    packet_handler: impl TcpGatewayPacketHandler,
    debug: bool,
) {
    let mut buf = crate::tcp_utils::allocated_read_buffer(None);

    loop {
        let mut payload_size = [0u8; 4];
        let read_result = read.read_exact(&mut payload_size).await;

        match read_result {
            Ok(result) => {
                if result != payload_size.len() {
                    println!("[1] TCP Gateway is disconnected");

                    break;
                }
            }
            Err(err) => {
                if debug {
                    println!(
                        "[1] Failed to read payload size from TCP Gateway at {}. Err: {:?}",
                        tcp_gateway.gateway_host.as_str(),
                        err
                    );
                }

                break;
            }
        }

        let decrypted = {
            let payload_size = u32::from_le_bytes(payload_size) as usize;

            if payload_size > MAX_PAYLOAD_SIZE {
                println!(
                    "[1] Failed to read payload size from TCP Gateway at {}. Max payload size is overflows, PayloadSize: {payload_size}",
                    tcp_gateway.gateway_host.as_str(),
                );

                break;
            }
            if payload_size > buf.len() {
                let mut dynamic_buffer =
                    crate::tcp_utils::allocated_read_buffer(Some(payload_size));

                let read_amount = read_buffer(&mut read, &mut dynamic_buffer, debug).await;
                if read_amount == 0 {
                    break;
                } else {
                    gateway_connection.in_per_second.add(read_amount);
                }

                let aes_encrypted_data = AesEncryptedDataRef::new(&dynamic_buffer);
                tcp_gateway.encryption.decrypt(&aes_encrypted_data)
            } else {
                let read_amount = read_buffer(&mut read, &mut buf[0..payload_size], debug).await;

                if read_amount == 0 {
                    break;
                } else {
                    gateway_connection.in_per_second.add(read_amount);
                }

                let aes_encrypted_data = AesEncryptedDataRef::new(&buf[0..payload_size]);
                tcp_gateway.encryption.decrypt(&aes_encrypted_data)
            }
        };

        if decrypted.is_err() {
            println!("TcpGateway is closing connection since Encryption key is wrong");
            break;
        }

        let decrypted = decrypted.unwrap();

        match TcpGatewayContract::parse(decrypted.as_slice()) {
            Ok(packet) => {
                if !packet.is_ping_or_pong() {
                    println!("Gateway Packet: {:?}", packet);
                }

                let now = DateTimeAsMicroseconds::now();
                gateway_connection.set_last_incoming_payload_time(now);
                if let Err(err) = packet_handler
                    .handle_packet(packet, &tcp_gateway, &gateway_connection)
                    .await
                {
                    println!(
                        "Failed to handle packet from TCP Gateway at {}. Err: {}",
                        tcp_gateway.gateway_host.as_str(),
                        err
                    );
                    break;
                }
            }
            Err(err) => {
                println!(
                    "Failed to parse packet from TCP Gateway at {}. Err: {:?}",
                    tcp_gateway.gateway_host.as_str(),
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

async fn read_buffer(read: &mut OwnedReadHalf, buffer: &mut [u8], debug: bool) -> usize {
    let read_result = read.read_exact(buffer).await;

    match read_result {
        Ok(result) => {
            if result != buffer.len() {
                if debug {
                    println!("[2] TCP Gateway is disconnected");
                }

                return 0;
            }

            return result + 4;
        }
        Err(err) => {
            if debug {
                println!(
                    "[2] Failed to read payload size from TCP Gateway. Err: {:?}",
                    err
                );
            }

            return 0;
        }
    };
}
