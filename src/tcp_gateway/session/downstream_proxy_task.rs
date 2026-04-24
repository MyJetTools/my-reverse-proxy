use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use bytes::BytesMut;
use rust_extensions::SliceOrVec;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::tcp_gateway::TcpGatewayContract;

use super::session_struct::Session;

pub const DEFAULT_DOWNSTREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

const READ_BUFFER_CAPACITY: usize = 64 * 1024;

type WriteFuture =
    Pin<Box<dyn Future<Output = (OwnedWriteHalf, std::io::Result<()>)> + Send>>;

/// Per-cid downstream proxy task.
///
/// Runs on the side of the gateway that received a `Connect{cid, host}` packet.
/// Opens a real TCP connection to `host`, then runs a single `tokio::select!`
/// loop with four arms: data_rx (gateway → downstream), downstream read
/// (downstream → gateway), in-flight write_fut, and an idle timer that fires
/// only when no traffic moves in either direction for the timeout window.
///
/// This task captures `gw_session_id` at spawn — if the gateway session
/// reconnects mid-flight, an outbound message will be filtered by id mismatch
/// in the new session's dispatcher and the task subsequently observes
/// `data_rx.recv() == None` (old slots were drained on teardown) and exits.
pub async fn downstream_proxy_task(
    cid: u32,
    host: String,
    connect_timeout: Duration,
    idle_timeout: Duration,
    mut data_rx: mpsc::Receiver<Vec<u8>>,
    session: Arc<Session>,
) {
    let _gw_session_id = session.session_id;

    // 1. Open downstream TCP with timeout.
    let downstream = match tokio::time::timeout(connect_timeout, TcpStream::connect(&host)).await {
        Ok(Ok(s)) => s,
        Ok(Err(err)) => {
            send_connection_error(&session, cid, &format!("connect: {:?}", err)).await;
            session.remove_slot(cid);
            return;
        }
        Err(_) => {
            send_connection_error(&session, cid, "connect timeout").await;
            session.remove_slot(cid);
            return;
        }
    };

    // 2. Notify peer the connect succeeded.
    let connected = TcpGatewayContract::Connected { connection_id: cid }
        .to_vec(&session.aes_key, false);
    if session.write_tx.send(connected).await.is_err() {
        session.remove_slot(cid);
        return;
    }

    // 3. Split downstream — both halves stay in this task.
    let (mut read_half, mut write_half_holder) = {
        let (r, w) = downstream.into_split();
        (r, Some(w))
    };

    let mut buffer: Vec<u8> = Vec::new();
    let mut read_buf = BytesMut::with_capacity(READ_BUFFER_CAPACITY);
    let mut write_fut: Option<WriteFuture> = None;

    let sleep = tokio::time::sleep(idle_timeout);
    tokio::pin!(sleep);

    let mut exit_reason: Option<&'static str> = None;

    loop {
        // Re-arm write_fut if idle and buffer non-empty.
        if write_fut.is_none() && !buffer.is_empty() {
            if let Some(mut w) = write_half_holder.take() {
                let batch = std::mem::take(&mut buffer);
                write_fut = Some(Box::pin(async move {
                    let res = w.write_all(&batch).await;
                    (w, res)
                }));
            }
        }

        let select_outcome = tokio::select! {
            msg = data_rx.recv() => {
                match msg {
                    Some(payload) => {
                        buffer.extend_from_slice(&payload);
                        SelectOutcome::Activity
                    }
                    None => {
                        SelectOutcome::Exit("gateway/peer closed cid")
                    }
                }
            }

            res = downstream_read(&mut read_half, &mut read_buf), if exit_reason.is_none() => {
                match res {
                    Ok(0) => SelectOutcome::Exit("downstream EOF"),
                    Ok(_) => {
                        // Drain ready bytes into a fresh allocation; reset buffer for next read.
                        let chunk: Vec<u8> = read_buf.split().to_vec();
                        let frame = TcpGatewayContract::BackwardPayload {
                            connection_id: cid,
                            payload: SliceOrVec::AsVec(chunk),
                        }
                        .to_vec(&session.aes_key, session.support_compression);
                        if session.write_tx.send(frame).await.is_err() {
                            SelectOutcome::Exit("gateway write_tx closed")
                        } else {
                            SelectOutcome::Activity
                        }
                    }
                    Err(_) => SelectOutcome::Exit("downstream read error"),
                }
            }

            res = async { write_fut.as_mut().unwrap().as_mut().await }, if write_fut.is_some() => {
                let (returned, io_res) = res;
                write_half_holder = Some(returned);
                write_fut = None;
                if io_res.is_err() {
                    SelectOutcome::Exit("downstream write error")
                } else {
                    SelectOutcome::Activity
                }
            }

            _ = &mut sleep => {
                SelectOutcome::Exit("disconnected by timeout")
            }
        };

        match select_outcome {
            SelectOutcome::Activity => {
                sleep.as_mut().reset(tokio::time::Instant::now() + idle_timeout);
            }
            SelectOutcome::Exit(reason) => {
                exit_reason = Some(reason);
                break;
            }
        }
    }

    // Drain a still-running write_fut.
    if let Some(fut) = write_fut.take() {
        let (returned, _) = fut.await;
        write_half_holder = Some(returned);
    }
    if let Some(mut w) = write_half_holder.take() {
        let _ = w.shutdown().await;
    }

    // Best-effort tell peer the cid is closing.
    if let Some(reason) = exit_reason {
        send_connection_error(&session, cid, reason).await;
    }

    session.remove_slot(cid);
}

enum SelectOutcome {
    Activity,
    Exit(&'static str),
}

async fn downstream_read(
    read_half: &mut OwnedReadHalf,
    buf: &mut BytesMut,
) -> std::io::Result<usize> {
    if buf.capacity() - buf.len() < 4096 {
        buf.reserve(READ_BUFFER_CAPACITY);
    }
    read_half.read_buf(buf).await
}

async fn send_connection_error(session: &Arc<Session>, cid: u32, error: &str) {
    let frame = TcpGatewayContract::ConnectionError {
        connection_id: cid,
        error,
    }
    .to_vec(&session.aes_key, false);
    let _ = session.write_tx.send(frame).await;
}
