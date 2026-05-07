use std::sync::Arc;

use encryption::{aes::AesKey, AesEncryptedDataRef};
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::{io::AsyncReadExt, net::tcp::OwnedReadHalf};

use super::*;

const MAX_PAYLOAD_SIZE: usize = 1024 * 1024 * 5;
const MAX_DECOMPRESSED_BATCH_SIZE: usize = 16 * 1024 * 1024;

pub async fn gateway_read_loop(
    tcp_gateway: Arc<TcpGatewayInner>,
    mut read: OwnedReadHalf,
    gateway_connection: Arc<TcpGatewayConnection>,
    packet_handler: impl TcpGatewayPacketHandler,
    debug: bool,
) {
    let mut buf = crate::tcp_utils::allocated_read_buffer(None);
    let mut payload_no: u64 = 0;
    let aes_key = gateway_connection.get_aes_key().clone();

    loop {
        payload_no += 1;
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
                    "[1] Failed to read payload size from TCP Gateway at {}. Payload no: {payload_no}. Max payload size is overflown, PayloadSize: {payload_size}",
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
                aes_key.decrypt(&aes_encrypted_data)
            } else {
                let read_amount = read_buffer(&mut read, &mut buf[0..payload_size], debug).await;

                if read_amount == 0 {
                    break;
                } else {
                    gateway_connection.in_per_second.add(read_amount);
                }

                let aes_encrypted_data = AesEncryptedDataRef::new(&buf[0..payload_size]);
                aes_key.decrypt(&aes_encrypted_data)
            }
        };

        if decrypted.is_err() {
            println!("TcpGateway is closing connection: decryption failed");
            break;
        }

        let decrypted = decrypted.unwrap();
        let decrypted_bytes = decrypted.as_slice();

        if decrypted_bytes.is_empty() {
            println!("TcpGateway received empty frame body");
            break;
        }

        if decrypted_bytes[0] == COMPRESSED_BATCH_PACKET_ID {
            if let Err(err) = handle_compressed_batch(
                decrypted_bytes,
                &tcp_gateway,
                &gateway_connection,
                &packet_handler,
            )
            .await
            {
                println!(
                    "TcpGateway compressed batch from {} failed: {err}",
                    tcp_gateway.gateway_host.as_str()
                );
                break;
            }
            gateway_connection.set_last_incoming_payload_time(DateTimeAsMicroseconds::now());
            continue;
        }

        match TcpGatewayContract::parse(decrypted_bytes) {
            Ok(packet) => {
                gateway_connection.set_last_incoming_payload_time(DateTimeAsMicroseconds::now());
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

    let gateway_id = gateway_connection.get_gateway_id();
    tcp_gateway.set_gateway_connection(gateway_id.as_str(), None);
    gateway_connection.disconnect_gateway().await;
}

async fn handle_compressed_batch(
    decrypted: &[u8],
    tcp_gateway: &Arc<TcpGatewayInner>,
    gateway_connection: &Arc<TcpGatewayConnection>,
    packet_handler: &impl TcpGatewayPacketHandler,
) -> Result<(), String> {
    if decrypted.len() < 2 {
        return Err("COMPRESSED_BATCH: missing algo byte".to_string());
    }
    let algo = decrypted[1];
    if algo != COMPRESSION_ALGO_ZSTD {
        return Err(format!("COMPRESSED_BATCH: unknown algo byte {algo}"));
    }

    let compressed = &decrypted[2..];
    let decompressed = zstd::stream::decode_all(compressed)
        .map_err(|err| format!("COMPRESSED_BATCH: zstd decompress failed: {err}"))?;

    if decompressed.len() > MAX_DECOMPRESSED_BATCH_SIZE {
        return Err(format!(
            "COMPRESSED_BATCH: decompressed size {} exceeds {} limit",
            decompressed.len(),
            MAX_DECOMPRESSED_BATCH_SIZE
        ));
    }

    let mut offset = 0usize;
    while offset + 4 <= decompressed.len() {
        let inner_len = u32::from_le_bytes([
            decompressed[offset],
            decompressed[offset + 1],
            decompressed[offset + 2],
            decompressed[offset + 3],
        ]) as usize;
        offset += 4;
        if offset + inner_len > decompressed.len() {
            return Err("COMPRESSED_BATCH: truncated inner frame".to_string());
        }
        let body = &decompressed[offset..offset + inner_len];
        offset += inner_len;

        if body.is_empty() {
            return Err("COMPRESSED_BATCH: empty inner frame".to_string());
        }
        if body[0] == COMPRESSED_BATCH_PACKET_ID {
            return Err("COMPRESSED_BATCH: nested batch is not allowed".to_string());
        }

        let packet = TcpGatewayContract::parse(body).map_err(|err| {
            format!("COMPRESSED_BATCH: failed to parse inner frame: {err}")
        })?;
        packet_handler
            .handle_packet(packet, tcp_gateway, gateway_connection)
            .await
            .map_err(|err| format!("COMPRESSED_BATCH: handler failed: {err}"))?;
    }

    if offset != decompressed.len() {
        return Err("COMPRESSED_BATCH: trailing bytes after last inner frame".to_string());
    }

    Ok(())
}

#[allow(dead_code)]
fn _aes_unused(_key: &AesKey) {}

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
