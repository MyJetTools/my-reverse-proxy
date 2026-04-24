use std::{sync::Arc, time::Duration};

use encryption::aes::AesKey;

use crate::network_stream::MyOwnedReadHalf;
use crate::tcp_gateway::TcpGatewayContract;

use super::frame_reader::FrameReader;
use super::handshake_replay_guard::HandshakeReplayGuard;

/// Outcome of a successful handshake — what we learned about the peer.
pub struct HandshakeOutcome {
    pub gateway_name: String,
    pub support_compression: bool,
    pub timestamp: i64,
}

/// Wait for an incoming `Handshake` frame from the peer with a timeout, then
/// validate its timestamp against `replay_guard` (rejects timestamps outside
/// the freshness window or already-seen ones).
pub async fn wait_handshake(
    read_half: &mut MyOwnedReadHalf,
    aes: &Arc<AesKey>,
    timeout: Duration,
    replay_guard: &Arc<HandshakeReplayGuard>,
) -> Result<HandshakeOutcome, String> {
    let fut = wait_handshake_inner(read_half, aes);
    let outcome = match tokio::time::timeout(timeout, fut).await {
        Ok(res) => res?,
        Err(_) => return Err(format!("Handshake timeout after {:?}", timeout)),
    };
    replay_guard.validate(outcome.timestamp)?;
    Ok(outcome)
}

async fn wait_handshake_inner(
    read_half: &mut MyOwnedReadHalf,
    aes: &Arc<AesKey>,
) -> Result<HandshakeOutcome, String> {
    let mut reader = FrameReader::new();

    loop {
        match reader.pump_bytes(read_half).await {
            Ok(0) => return Err("Peer closed before handshake".to_string()),
            Ok(_) => {}
            Err(err) => return Err(format!("Read error during handshake: {:?}", err)),
        }

        match reader.try_next_frame(aes)? {
            Some(body) => {
                let packet = TcpGatewayContract::parse(&body)?;
                match packet {
                    TcpGatewayContract::Handshake {
                        timestamp,
                        support_compression,
                        gateway_name,
                    } => {
                        return Ok(HandshakeOutcome {
                            gateway_name: gateway_name.to_string(),
                            support_compression,
                            timestamp,
                        });
                    }
                    other => {
                        return Err(format!(
                            "Expected Handshake, got: {:?}",
                            other
                        ));
                    }
                }
            }
            None => continue,
        }
    }
}

/// Encode a `Handshake` outgoing frame ready to push into the write channel.
pub fn encode_handshake(
    gateway_name: &str,
    support_compression: bool,
    timestamp: i64,
    aes: &AesKey,
) -> Vec<u8> {
    TcpGatewayContract::Handshake {
        timestamp,
        support_compression,
        gateway_name,
    }
    .to_vec(aes, false)
}
