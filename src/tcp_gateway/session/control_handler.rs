use std::{sync::Arc, time::Duration};

use tokio::sync::mpsc;

use crate::tcp_gateway::TcpGatewayContract;

use super::downstream_proxy_task::{downstream_proxy_task, DEFAULT_DOWNSTREAM_IDLE_TIMEOUT};
use super::session_struct::Session;

#[derive(Clone)]
pub struct ControlHandlerConfig {
    pub ping_interval: Duration,
    pub dead_timeout: Duration,
    pub watchdog_tick: Duration,

    pub data_channel_capacity: usize,
    pub downstream_connect_timeout: Duration,
    pub downstream_idle_timeout: Duration,
}

impl Default for ControlHandlerConfig {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(3),
            dead_timeout: Duration::from_secs(9),
            watchdog_tick: Duration::from_secs(1),

            data_channel_capacity: 256,
            downstream_connect_timeout: Duration::from_secs(10),
            downstream_idle_timeout: DEFAULT_DOWNSTREAM_IDLE_TIMEOUT,
        }
    }
}

/// The control plane handler for one gateway session.
///
/// Owns:
/// - the receive end of the per-session `control_tx` channel (raw decrypted
///   frame bodies of control packets — `read_loop` puts them here),
/// - the watchdog ticker that tracks `session.last_incoming_micros`.
///
/// Returns when the channel closes OR the watchdog declares the session dead.
/// Exiting causes `supervisor_loop` to observe a completed task and reconnect.
pub async fn control_handler(
    session: Arc<Session>,
    mut control_rx: mpsc::Receiver<Vec<u8>>,
    cfg: ControlHandlerConfig,
) {
    let mut ticker = tokio::time::interval(cfg.watchdog_tick);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = control_rx.recv() => {
                match msg {
                    Some(body) => {
                        if let Err(err) = handle_control_packet(&body, &session, &cfg).await {
                            eprintln!(
                                "control_handler[session={}]: {}",
                                session.session_id, err
                            );
                        }
                    }
                    None => {
                        // read_loop closed control_tx — session is unwinding.
                        break;
                    }
                }
            }

            _ = ticker.tick() => {
                let idle_micros = session.idle_micros();
                let dead_micros = cfg.dead_timeout.as_micros() as i64;
                let ping_micros = cfg.ping_interval.as_micros() as i64;

                if idle_micros > dead_micros {
                    eprintln!(
                        "control_handler[session={}]: dead by idle ({}us > {}us); tearing down",
                        session.session_id, idle_micros, dead_micros
                    );
                    break;
                }
                if idle_micros > ping_micros {
                    let ping = TcpGatewayContract::Ping.to_vec(&session.aes_key, false);
                    let _ = session.write_tx.send(ping).await;
                }
            }
        }
    }
}

async fn handle_control_packet(
    body: &[u8],
    session: &Arc<Session>,
    cfg: &ControlHandlerConfig,
) -> Result<(), String> {
    let packet = TcpGatewayContract::parse(body)?;

    match packet {
        TcpGatewayContract::Ping => {
            let pong = TcpGatewayContract::Pong.to_vec(&session.aes_key, false);
            let _ = session.write_tx.send(pong).await;
        }
        TcpGatewayContract::Pong => {
            // last_incoming was already updated in read_loop on frame receipt.
        }
        TcpGatewayContract::UpdatePingTime { duration: _ } => {
            // Phase 2: integration with metrics is post-rewrite.
        }
        TcpGatewayContract::Handshake { .. } => {
            // Handshake handled at the supervisor level before control_handler runs.
            // A late Handshake is a protocol violation — drop with a log.
            return Err("Handshake received after session start".into());
        }
        TcpGatewayContract::Connect {
            connection_id,
            timeout,
            remote_host,
        } => {
            let host = remote_host.to_string();
            let (data_tx, data_rx) = mpsc::channel::<Vec<u8>>(cfg.data_channel_capacity);
            session.insert_slot(connection_id, data_tx);
            let session_clone = session.clone();
            let connect_timeout = if timeout.as_secs() > 0 {
                timeout
            } else {
                cfg.downstream_connect_timeout
            };
            let idle = cfg.downstream_idle_timeout;
            tokio::spawn(downstream_proxy_task(
                connection_id,
                host,
                connect_timeout,
                idle,
                data_rx,
                session_clone,
            ));
        }
        TcpGatewayContract::Connected { connection_id } => {
            if let Some(reply_to) = session.take_pending(connection_id) {
                let _ = reply_to.send(Ok(()));
            } else {
                eprintln!(
                    "control_handler[session={}]: Connected for unknown cid={}",
                    session.session_id, connection_id
                );
            }
        }
        TcpGatewayContract::ConnectionError {
            connection_id,
            error,
        } => {
            let err_msg = error.to_string();
            if let Some(reply_to) = session.take_pending(connection_id) {
                let _ = reply_to.send(Err(err_msg.clone()));
            }
            if let Some(slot_tx) = session.remove_slot(connection_id) {
                drop(slot_tx);
            }
        }
        TcpGatewayContract::SyncSslCertificates { .. }
        | TcpGatewayContract::SyncSslCertificatesRequest { .. }
        | TcpGatewayContract::SyncSslCertificateNotFound { .. }
        | TcpGatewayContract::GetFileRequest { .. }
        | TcpGatewayContract::GetFileResponse { .. } => {
            // Phase 2: pass-through; integration with SSL/file storage will be
            // wired in Phase 3 alongside the AppContext rewire.
        }
        TcpGatewayContract::ForwardPayload { .. } | TcpGatewayContract::BackwardPayload { .. } => {
            // Should never reach control_handler — read_loop classifies these as
            // data-path packets and sends them directly to slots.
            return Err("data-plane packet leaked into control_tx".into());
        }
    }

    Ok(())
}
