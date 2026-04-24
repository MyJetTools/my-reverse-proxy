// TECH DEBT: This module contains the next-generation gateway session/supervisor
// stack (Session, control_handler, downstream_proxy_task, GatewaySupervisor,
// HandshakeReplayGuard, etc.). It is fully tested in isolation but **not yet
// wired** into TcpGatewayClient / TcpGatewayServer / TcpGatewayProxyForwardStream.
//
// The lost-wakeup write-loop bug fix in TcpConnectionInner already reuses the
// race-free `gateway_write_loop` from this module; everything else here is
// staged for a follow-up "Phase 3B" rewrite that:
//  - replaces TcpGatewayClient::connection_loop with GatewaySupervisor
//  - replaces TcpGatewayServer::connection_loop with accept_gateway_session
//  - migrates packet_handler logic into a callback-driven control_handler
//  - migrates TcpGatewayProxyForwardStream onto Session.write_tx + slots
//  - enables HandshakeReplayGuard and server-side handshake displacement
//  - deletes tcp_connection_inner/, gateway_read_loop, packet_handler/scripts
//
// Until then, dead-code / unused-import warnings on the staged items are
// suppressed module-wide so the rest of the codebase stays clean.
#![allow(dead_code, unused_imports)]

mod control_handler;
mod downstream_proxy_task;
mod frame_reader;
mod handshake;
mod handshake_replay_guard;
mod proxy_handle;
mod read_loop;
mod session_struct;
mod supervisor;
mod write_loop;

#[cfg(test)]
mod tests;

pub use control_handler::{control_handler, ControlHandlerConfig};
pub use downstream_proxy_task::{downstream_proxy_task, DEFAULT_DOWNSTREAM_IDLE_TIMEOUT};
pub use frame_reader::{FrameReader, MAX_PAYLOAD_SIZE};
pub use handshake::{encode_handshake, wait_handshake, HandshakeOutcome};
pub use handshake_replay_guard::HandshakeReplayGuard;
pub use proxy_handle::ProxyHandle;
pub use read_loop::gateway_read_loop;
pub use session_struct::{ConnectReplyTx, Session};
pub use supervisor::{accept_gateway_session, GatewaySupervisor, SessionTasks, SupervisorConfig};
pub use write_loop::gateway_write_loop;
