use std::{future::Future, pin::Pin, sync::Arc};

use encryption::aes::AesKey;
use tokio::sync::mpsc;

use crate::network_stream::{MyOwnedWriteHalf, NetworkStreamWritePart};
use crate::tcp_gateway::{
    encrypt_frame, COMPRESSED_BATCH_PACKET_ID, COMPRESSION_ALGO_ZSTD,
};

const ZSTD_LEVEL: i32 = 3;

type WriteFuture =
    Pin<Box<dyn Future<Output = (MyOwnedWriteHalf, Result<(), std::io::Error>)> + Send>>;

/// Writer task for a gateway TCP connection.
///
/// Producers push **plaintext** inner frames `[u32 LEN][u8 TYPE][PAYLOAD]`
/// into `rx`. The writer accumulates them in a `Vec<u8>` while the previous
/// `write_all` is still in flight. On every flush boundary the accumulator
/// is drained and either:
///
/// - encrypted frame-by-frame and written as concatenated wire frames
///   (`compress_outbound = false`), or
/// - zstd-compressed as a whole, wrapped as a single `COMPRESSED_BATCH`
///   frame, then encrypted and written (`compress_outbound = true`).
pub async fn gateway_write_loop(
    write_half: MyOwnedWriteHalf,
    mut rx: mpsc::Receiver<Vec<u8>>,
    aes_key: Arc<AesKey>,
    compress_outbound: bool,
) {
    let mut buffer: Vec<u8> = Vec::new();
    let mut write_fut: Option<WriteFuture> = None;
    let mut write_half_holder: Option<MyOwnedWriteHalf> = Some(write_half);

    loop {
        if write_fut.is_none() && !buffer.is_empty() {
            let plaintext = std::mem::take(&mut buffer);
            let wire = build_wire(&plaintext, &aes_key, compress_outbound);
            let mut owned_write_half = write_half_holder.take().expect("write_half was borrowed");
            write_fut = Some(Box::pin(async move {
                let res = owned_write_half.write_to_socket(&wire).await;
                (owned_write_half, res)
            }));
        }

        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(payload) => buffer.extend_from_slice(&payload),
                    None => break,
                }
            }

            res = async { write_fut.as_mut().unwrap().as_mut().await }, if write_fut.is_some() => {
                let (returned_half, io_res) = res;
                write_half_holder = Some(returned_half);
                write_fut = None;
                if let Err(err) = io_res {
                    eprintln!("gateway_write_loop: write failure: {:?}", err);
                    break;
                }
            }
        }
    }

    if let Some(fut) = write_fut.take() {
        let (returned_half, _) = fut.await;
        write_half_holder = Some(returned_half);
    }
    if let Some(mut half) = write_half_holder.take() {
        half.shutdown_socket().await;
    }
}

fn build_wire(plaintext: &[u8], aes: &AesKey, compress_outbound: bool) -> Vec<u8> {
    if compress_outbound {
        encode_compressed_batch(plaintext, aes)
    } else {
        encode_individual_frames(plaintext, aes)
    }
}

fn encode_individual_frames(plaintext: &[u8], aes: &AesKey) -> Vec<u8> {
    let mut out = Vec::with_capacity(plaintext.len() + 64);
    let mut offset = 0usize;
    while offset + 4 <= plaintext.len() {
        let inner_len = u32::from_le_bytes([
            plaintext[offset],
            plaintext[offset + 1],
            plaintext[offset + 2],
            plaintext[offset + 3],
        ]) as usize;
        offset += 4;
        if offset + inner_len > plaintext.len() {
            eprintln!(
                "gateway_write_loop: truncated inner frame in accumulator (need {inner_len}, have {})",
                plaintext.len() - offset
            );
            return out;
        }
        let body = &plaintext[offset..offset + inner_len];
        offset += inner_len;
        out.extend_from_slice(&encrypt_frame(body, aes));
    }
    out
}

fn encode_compressed_batch(plaintext: &[u8], aes: &AesKey) -> Vec<u8> {
    let compressed = match zstd::stream::encode_all(plaintext, ZSTD_LEVEL) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("gateway_write_loop: zstd compression failed: {err}");
            return encode_individual_frames(plaintext, aes);
        }
    };

    let mut body = Vec::with_capacity(2 + compressed.len());
    body.push(COMPRESSED_BATCH_PACKET_ID);
    body.push(COMPRESSION_ALGO_ZSTD);
    body.extend_from_slice(&compressed);
    encrypt_frame(&body, aes)
}
