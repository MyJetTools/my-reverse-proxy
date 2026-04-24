use std::sync::Arc;

use crate::network_stream::MyOwnedReadHalf;

use super::frame_reader::FrameReader;
use super::session_struct::Session;

// Packet IDs that take the data-plane fast path (direct dispatch to slots).
// Kept locally to avoid making the gateway_contracts constants public.
const PACKET_ID_SEND_PAYLOAD: u8 = 6;
const PACKET_ID_RECEIVE_PAYLOAD: u8 = 7;

/// The read side of a gateway session.
///
/// Reads bytes, accumulates into a `FrameReader`, extracts complete encrypted
/// frames one-by-one, decrypts each, and dispatches by inspecting only the
/// first byte (packet kind):
///
/// - **Data plane** (`SEND_PAYLOAD` / `RECEIVE_PAYLOAD`): parse `cid` and
///   payload inline, look up `session.slots[cid]`, push payload via the
///   per-slot `send().await` (TCP-like backpressure — a slow forwarded
///   connection blocks dispatch on this gateway).
/// - **Control plane** (everything else): the entire decrypted body is forwarded
///   verbatim to `control_tx`. The `control_handler` task parses it.
///
/// `session.last_incoming_micros` is updated on every successfully extracted
/// frame, regardless of kind — any traffic counts as liveness.
pub async fn gateway_read_loop(mut read_half: MyOwnedReadHalf, session: Arc<Session>) {
    let mut reader = FrameReader::new();

    loop {
        match reader.pump_bytes(&mut read_half).await {
            Ok(0) => {
                eprintln!(
                    "gateway_read_loop[session={}]: peer closed",
                    session.session_id
                );
                break;
            }
            Ok(_) => {}
            Err(err) => {
                eprintln!(
                    "gateway_read_loop[session={}]: read error: {:?}",
                    session.session_id, err
                );
                break;
            }
        }

        loop {
            let body = match reader.try_next_frame(&session.aes_key) {
                Ok(Some(b)) => b,
                Ok(None) => break,
                Err(err) => {
                    eprintln!(
                        "gateway_read_loop[session={}]: frame decode error: {}",
                        session.session_id, err
                    );
                    return;
                }
            };

            session.mark_incoming();

            if body.is_empty() {
                continue;
            }

            let packet_kind = body[0];
            match packet_kind {
                PACKET_ID_SEND_PAYLOAD | PACKET_ID_RECEIVE_PAYLOAD => {
                    if let Err(err) = dispatch_data(&body, &session).await {
                        eprintln!(
                            "gateway_read_loop[session={}]: data dispatch: {}",
                            session.session_id, err
                        );
                    }
                }
                _ => {
                    if session.control_tx.send(body).await.is_err() {
                        // control_handler died — session is unwinding.
                        return;
                    }
                }
            }
        }
    }
}

async fn dispatch_data(body: &[u8], session: &Arc<Session>) -> Result<(), String> {
    if body.len() < 6 {
        return Err("data frame too short".into());
    }
    let cid = u32::from_le_bytes([body[1], body[2], body[3], body[4]]);
    let compressed = body[5] == 1;

    let payload =
        crate::tcp_gateway::decompress_payload(&body[6..], compressed).map_err(|e| e)?;
    let payload_vec = payload.as_slice().to_vec();

    let Some(slot_tx) = session.get_slot(cid) else {
        // Unknown cid — silently drop (peer may have raced a close, or session
        // identifier mismatch handled at higher level).
        return Ok(());
    };

    if slot_tx.send(payload_vec).await.is_err() {
        // The per-cid task has gone away; remove the orphaned slot.
        session.remove_slot(cid);
    }
    Ok(())
}
