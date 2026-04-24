use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use arc_swap::ArcSwapOption;
use encryption::aes::AesKey;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::network_stream::MyOwnedWriteHalf;

use super::control_handler::{control_handler, ControlHandlerConfig};
use super::handshake::{encode_handshake, wait_handshake, HandshakeOutcome};
use super::handshake_replay_guard::HandshakeReplayGuard;
use super::read_loop::gateway_read_loop;
use super::session_struct::Session;
use super::write_loop::gateway_write_loop;

/// Configuration for the client-side gateway supervisor.
pub struct SupervisorConfig {
    pub gateway_id: String,
    pub remote_host: String,
    pub aes_key: Arc<AesKey>,
    pub support_compression: bool,

    pub connect_timeout: Duration,
    pub reconnect_delay: Duration,
    pub write_channel_capacity: usize,
    pub control_channel_capacity: usize,

    pub control_handler: ControlHandlerConfig,
    pub replay_guard: Arc<HandshakeReplayGuard>,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            gateway_id: String::new(),
            remote_host: String::new(),
            aes_key: Arc::new(AesKey::new(b"")),
            support_compression: false,

            connect_timeout: Duration::from_secs(10),
            reconnect_delay: Duration::from_secs(3),
            write_channel_capacity: 1024,
            control_channel_capacity: 256,

            control_handler: ControlHandlerConfig::default(),
            replay_guard: Arc::new(HandshakeReplayGuard::default_window()),
        }
    }
}

/// Long-lived handle stored in AppContext. Owns the supervisor task and exposes
/// the currently-active `Arc<Session>` (or `None` while disconnected) via
/// `ArcSwapOption`.
pub struct GatewaySupervisor {
    pub active: Arc<ArcSwapOption<Session>>,
    running: Arc<AtomicBool>,
    next_session_id: Arc<AtomicU64>,
}

impl GatewaySupervisor {
    /// Spawn a supervisor that maintains a client-side gateway connection,
    /// reconnecting on every disconnect with the configured backoff.
    pub fn spawn(cfg: SupervisorConfig) -> Self {
        let active: Arc<ArcSwapOption<Session>> = Arc::new(ArcSwapOption::const_empty());
        let running = Arc::new(AtomicBool::new(true));
        let next_session_id = Arc::new(AtomicU64::new(1));

        let result = Self {
            active: active.clone(),
            running: running.clone(),
            next_session_id: next_session_id.clone(),
        };

        tokio::spawn(supervisor_loop(cfg, active, running, next_session_id));
        result
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn current(&self) -> Option<Arc<Session>> {
        self.active.load_full()
    }
}

impl Drop for GatewaySupervisor {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn supervisor_loop(
    cfg: SupervisorConfig,
    active: Arc<ArcSwapOption<Session>>,
    running: Arc<AtomicBool>,
    next_session_id: Arc<AtomicU64>,
) {
    while running.load(Ordering::Relaxed) {
        match attempt_connect(&cfg, &next_session_id).await {
            Ok(SessionTasks {
                session,
                read_handle,
                write_handle,
                control_handle,
            }) => {
                active.store(Some(session.clone()));

                tokio::select! {
                    _ = read_handle => {}
                    _ = write_handle => {}
                    _ = control_handle => {}
                }

                active.store(None);
                for (_, reply_to) in session.drain_pending() {
                    let _ = reply_to.send(Err("Gateway disconnected".to_string()));
                }
                session.drop_all_slots();
            }
            Err(err) => {
                eprintln!(
                    "GatewaySupervisor[{}]: connect failed: {}",
                    cfg.gateway_id, err
                );
            }
        }

        if running.load(Ordering::Relaxed) {
            tokio::time::sleep(cfg.reconnect_delay).await;
        }
    }
}

pub struct SessionTasks {
    pub session: Arc<Session>,
    pub read_handle: tokio::task::JoinHandle<()>,
    pub write_handle: tokio::task::JoinHandle<()>,
    pub control_handle: tokio::task::JoinHandle<()>,
}

/// Server-side counterpart of `attempt_connect`. Given an already-accepted
/// `TcpStream`, completes a handshake exchange and spawns the same
/// read/write/control task triplet. Caller is responsible for choosing a
/// `session_id` (it should be globally unique for this process).
pub async fn accept_gateway_session(
    stream: TcpStream,
    session_id: u64,
    gateway_id: &str,
    aes_key: Arc<AesKey>,
    support_compression: bool,
    handshake_timeout: Duration,
    write_channel_capacity: usize,
    control_channel_capacity: usize,
    control_cfg: ControlHandlerConfig,
    replay_guard: Arc<HandshakeReplayGuard>,
) -> Result<SessionTasks, String> {
    let (read, write) = stream.into_split();
    let mut read_half = crate::network_stream::MyOwnedReadHalf::Tcp(read);
    let write_half: MyOwnedWriteHalf = MyOwnedWriteHalf::Tcp(write);

    let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(write_channel_capacity);
    let (control_tx, control_rx) = mpsc::channel::<Vec<u8>>(control_channel_capacity);

    let session = Arc::new(Session::new(
        session_id,
        write_tx.clone(),
        control_tx,
        aes_key.clone(),
        support_compression,
    ));

    let write_handle = tokio::spawn(gateway_write_loop(write_half, write_rx));

    // Wait for peer's Handshake first — server is reactive.
    let peer_hs = wait_handshake(
        &mut read_half,
        &aes_key,
        handshake_timeout,
        &replay_guard,
    )
    .await?;

    // Echo back our own Handshake so the peer also moves past it.
    let now_micros = DateTimeAsMicroseconds::now().unix_microseconds;
    let our_hs = encode_handshake(gateway_id, support_compression, now_micros, &aes_key);
    if write_tx.send(our_hs).await.is_err() {
        return Err("write_loop died before handshake echo".to_string());
    }

    eprintln!(
        "accept_gateway_session[{}]: handshake from peer name={} compression={} ts={}",
        gateway_id, peer_hs.gateway_name, peer_hs.support_compression, peer_hs.timestamp,
    );

    let read_handle = tokio::spawn(gateway_read_loop(read_half, session.clone()));
    let control_handle = tokio::spawn(control_handler(session.clone(), control_rx, control_cfg));

    Ok(SessionTasks {
        session,
        read_handle,
        write_handle,
        control_handle,
    })
}

async fn attempt_connect(
    cfg: &SupervisorConfig,
    next_session_id: &Arc<AtomicU64>,
) -> Result<SessionTasks, String> {
    let connect_fut = TcpStream::connect(&cfg.remote_host);
    let stream = match tokio::time::timeout(cfg.connect_timeout, connect_fut).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(format!("tcp connect: {:?}", e)),
        Err(_) => return Err(format!("tcp connect timeout {:?}", cfg.connect_timeout)),
    };

    let (read, write) = stream.into_split();
    let mut read_half = crate::network_stream::MyOwnedReadHalf::Tcp(read);
    let write_half: MyOwnedWriteHalf = MyOwnedWriteHalf::Tcp(write);

    let session_id = next_session_id.fetch_add(1, Ordering::Relaxed);
    let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(cfg.write_channel_capacity);
    let (control_tx, control_rx) = mpsc::channel::<Vec<u8>>(cfg.control_channel_capacity);

    let session = Arc::new(Session::new(
        session_id,
        write_tx.clone(),
        control_tx,
        cfg.aes_key.clone(),
        cfg.support_compression,
    ));

    let write_handle = tokio::spawn(gateway_write_loop(write_half, write_rx));

    let now_micros = DateTimeAsMicroseconds::now().unix_microseconds;
    let handshake = encode_handshake(
        &cfg.gateway_id,
        cfg.support_compression,
        now_micros,
        &cfg.aes_key,
    );
    if write_tx.send(handshake).await.is_err() {
        return Err("write_loop died before handshake send".to_string());
    }

    let handshake_outcome: HandshakeOutcome = wait_handshake(
        &mut read_half,
        &cfg.aes_key,
        cfg.connect_timeout,
        &cfg.replay_guard,
    )
    .await?;

    eprintln!(
        "GatewaySupervisor[{}]: handshake from peer name={} compression={} ts={}",
        cfg.gateway_id,
        handshake_outcome.gateway_name,
        handshake_outcome.support_compression,
        handshake_outcome.timestamp,
    );

    let read_handle = tokio::spawn(gateway_read_loop(read_half, session.clone()));
    let control_handle = tokio::spawn(control_handler(
        session.clone(),
        control_rx,
        cfg.control_handler.clone(),
    ));

    Ok(SessionTasks {
        session,
        read_handle,
        write_handle,
        control_handle,
    })
}
